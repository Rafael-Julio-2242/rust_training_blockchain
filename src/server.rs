use std::{collections::{HashMap, HashSet}, io::{Read, Write}, net::TcpStream, sync::{Arc, Mutex}};
use bincode::deserialize;
use failure::format_err;
use log::info;
use serde::{Deserialize, Serialize};
use crate::{block::Block, transaction::Transaction, utxoset::UTXOSet};
use crate::error::Result;

const KNOWN_NODE1: &str = "localhost:3000";
const CMD_LEN: usize = 12;
const VERSION: i32 = 1;

pub struct Server {
    node_address: String,
    mining_address: String,
    inner: Arc<Mutex<ServerInner>>
}

pub struct ServerInner {
    known_nodes: HashSet<String>,
    utxo: UTXOSet,
    blocks_in_transit: Vec<String>,
    mempool: HashMap<String, Transaction>
}


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Blockmsg {
    addr_from: String,
    block: Block
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GetBlockmsg {
    addr_from: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GetDatamsg {
    addr_from: String,
    kind: String,
    id: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Invmsg {
    addr_from: String,
    kind: String,
    items: Vec<String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Txmsg {
    addr_from: String,
    transaction: Transaction
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Versionmsg {
    addr_from: String,
    version: i32,
    best_height: i32
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    Addr(Vec<String>),
    Version(Versionmsg),
    Tx(Txmsg),
    GetData(GetDatamsg),
    GetBlock(GetBlockmsg),
    Inv(Invmsg),
    Block(Blockmsg)
}

struct CmdTypes {
    addr_type: &[u8],
}

impl Server {
    pub fn new(port: &str, miner_address: &str, utxo: UTXOSet) -> Result<Server> {

        let mut node_set = HashSet::new();
        node_set.insert(String::from(KNOWN_NODE1));
        Ok(
            Server {
                node_address: String::from("localhost:") + port,
                mining_address: miner_address.to_string(),
                inner: Arc::new(Mutex::new( ServerInner {
                    known_nodes: node_set,
                    utxo,
                    blocks_in_transit: Vec::new(),
                    mempool: HashMap::new(),
                })),
            }
        )
    }

    fn handle_connection(&self, mut stream: TcpStream) -> Result<()> {
        let mut buffer = Vec::new();
        let count = stream.read_to_end(&mut buffer)?;
        info!("Accept request: length {}", count);

        let cmd = bytes_to_cmd(&buffer)?;

        match cmd {
            Message::Addr(data) => self.handle_addr(data)?,
            Message::Block(data) => self.handle_block(data)?,
            Message::Inv(data) => self.handle_inv(data)?,
            Message::GetBlock(data) => self.handle_get_blocks(data)?,
            Message::GetData(data) => self.handle_get_data(data)?,
            Message::Tx(data) => self.handle_tx(data)?,
            Message::Version(data) => self.handle_version(data)?
        }

        Ok(())

    }

    fn handle_addr(&self, msg: Vec<String>) -> Result<()> {
        info!("receive address msg: {:?}", msg);
        for node in msg {
            self.add_nodes(&node);
        }

        Ok(())
    }

    fn handle_block(&self, msg: Blockmsg) -> Result<()> {
        info!(
            "receive block msg: {}, {}",
            msg.addr_from,
            msg.block.get_hash()
        );
        self.add_block(msg.block)?;

        let mut in_transit = self.get_in_transit()?;
        if in_transit.len() > 0 {
            let block_hash = &in_transit[0];
            self.send_get_data(&msg.addr_from, "block", block_hash)?;
            in_transit.remove(0);   
            self.replace_in_transit(in_transit);
        } else {
            self.utxo_reindex()?;
        }
        Ok(())

    }

    fn add_nodes(&self, addr: &str) {
        self.inner
            .lock()
            .unwrap()
            .known_nodes
            .insert(String::from(addr));
    }

    fn send_get_blocks(&self, addr: &str) -> Result<()> {
        info!("send get blocks message to: {}", addr);

        let data = GetBlockmsg {
            addr_from: self.node_address.clone()
        };

        let data = bincode::serialize(&(cmd_to_bytes("getblocks"), data))?;
        self.send_data(addr, &data)

    }

    fn send_get_data(&self, addr: &str, kind: &str, id: &str) -> Result<()> {
        info!(
            "send get data message to: {} kind: {} id: {}",
            addr, kind, id
        );

        let data = GetDatamsg {
            addr_from: self.node_address.clone(),
            kind: String::from(kind),
            id: String::from(id)
        };
        let data = bincode::serialize(&(cmd_to_bytes("getdata"), data))?;
        self.send_data(addr, &data)

    }

    fn send_version(&self, addr: &str) -> Result<()> {

        info!("send version to: {}", addr);
        let data = Versionmsg {
            addr_from: self.node_address.clone(),
            best_height: self.get_best_height()?,
            version: VERSION
        };
        let data = bincode::serialize(&(cmd_to_bytes("version"), data))?;
        self.send_data(addr, &data)

    }

    fn send_tx(&self, addr: &str, tx: &Transaction) -> Result<()> {
        info!("send tx to: {} txid: {}", addr, &tx.id);

        let data = Txmsg {
            addr_from: self.node_address.clone(),
            transaction: tx.clone()
        };
        let data = bincode::serialize(&(cmd_to_bytes("tx"), data))?;
        self.send_data(addr, &data)

    }

    fn send_inv(&self, addr: &str, kind: &str, items: Vec<String>) -> Result<()> {
        info!(
            "Send inv message to: {} kind: {} data: {:?}",
            addr, kind, items
        );

        let data = Invmsg {
            addr_from: self.node_address.clone(),
            kind: kind.to_string(),
            items
        };
        let data = bincode::serialize(&(cmd_to_bytes("inv"), data))?;
        self.send_data(addr, &data)
    }

    fn send_block(&self, addr: &str, b: &Block) -> Result<()> {
        info!("Send block data to: {} block hash: {}", addr, b.get_hash());

        let data = Blockmsg {
            addr_from: self.node_address.clone(),
            block: b.clone()
        };
        let data = bincode::serialize(&(cmd_to_bytes("block"), data))?;
        self.send_data(addr, &data)
    }

    fn send_addr(&self, addr: &str) -> Result<()> {
        info!("Send address info to: {}", addr);

        let nodes = self.get_known_nodes();
        let data = bincode::serialize(&(cmd_to_bytes("addr"), nodes))?;
        self.send_data(addr, &data)

    }

    fn get_known_nodes(&self) -> HashSet<String> {
        self.inner.lock().unwrap().known_nodes.clone()
    }

    fn send_data(&self, addr: &str, data: &[u8]) -> Result<()> {
        if addr == &self.node_address {
            return Ok(());
        }

        let mut stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(_) => {
                self.remove_node(addr);
                return Ok(());
            }
        };

        stream.write(data)?;
        
        info!("Data send successfully");
        Ok(())

    }

    fn remove_node(&self, addr: &str) {
        self.inner.lock().unwrap().known_nodes.remove(addr);
    }


}

fn cmd_to_bytes(cmd: &str) -> [u8; CMD_LEN] {
    let mut data = [0; CMD_LEN];
    for (i, d) in cmd.as_bytes().iter().enumerate() {
        data[i] = *d;
    }
    data
}

// cargo run addr data
fn bytes_to_cmd(bytes: &[u8]) -> Result<Message> {
    let mut cmd = Vec::new();
    let cmd_bytes = &bytes[..CMD_LEN];
    let data = &bytes[CMD_LEN..];
    for b in cmd_bytes {
        if 0 as u8 != *b {
            cmd.push(*b);
        }
    }
    info!("cmd: {}", String::from_utf8(cmd.clone())? );

    if cmd == "addr".as_bytes() {
        let data: Vec<String> = deserialize(data)?;
        Ok(Message::Addr(data))
    } else if cmd == "block".as_bytes() {
        let data: Blockmsg = deserialize(data)?;
        Ok(Message::Block(data))
    } else if cmd == "inv".as_bytes() {
        let data = deserialize(data)?;
        Ok(Message::Inv(data))
    } else if cmd == "getblocks".as_bytes() {
        let data = deserialize(data)?;
        Ok(Message::GetBlock(data))
    } else if cmd == "getdata".as_bytes() {
        let data = deserialize(data)?;
        Ok(Message::GetData(data))
    } else if cmd == "tx".as_bytes() {
        let data = deserialize(data)?;
        Ok(Message::Tx(data))
    } else if cmd == "version".as_bytes() {
        let data = deserialize(data)?;
        Ok(Message::Version(data))
    } else {
        Err(format_err!("Unknown command in the server"))
    }
    

}