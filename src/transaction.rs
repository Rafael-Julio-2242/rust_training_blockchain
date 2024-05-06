
use std::collections::HashMap;

use crypto::{digest::Digest, sha2::Sha256};
use failure::Error;
use failure::format_err;
use log::error;
use serde::{Deserialize, Serialize};
use crate::tx::TXInput;
use crate::tx::TXOutput;
use crate::{blockchain::Blockchain, error::Result};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub vin: Vec<TXInput>,
    pub vout: Vec<TXOutput>
}



impl Transaction {

   
    /// New UTXO creates a new transaction
    pub fn new_UTXO(from: &str, to: &str, amount: i32, bc: &Blockchain) -> Result<Transaction> {
        let mut vin = Vec::new();
        let acc_v = bc.find_spendable_outputs(from, amount);


        if acc_v.0 < amount {
            error!("Not enough funds");
            return Err(format_err!("Not Enough balance: current balance {}", acc_v.0));
        }

        for tx in acc_v.1 {
            for out in tx.1 {
                let input = TXInput {
                    txid: tx.0.clone(),
                    vout: out,
                    script_sig: String::from(from)
                };
                vin.push(input);
            }
        }

        let mut vout = vec![TXOutput {
            value: amount,
            script_pub_key: String::from(to),
        }];

        if acc_v.0 > amount {
            vout.push(TXOutput {
                value: acc_v.0 - amount,
                script_pub_key: String::from(from)
            })
        }

        let mut tx = Transaction {
            id: String::new(),
            vin,
            vout
        };

        tx.set_id()?;
        Ok(tx)
    }

    pub fn new_coinbase(to: String, mut data: String) -> Result<Transaction> {

        if data == String::from("") {
            data += &format!("Reward to '{}'", to);
        }

        let mut tx = Transaction {
            id: String::new(),
            vin: vec![TXInput {
                txid: String::new(),
                vout: -1,
                script_sig: data
            }],
            vout: vec![TXOutput {
                value: 100,
                script_pub_key: to
            }]
        };

        tx.set_id()?;
        Ok(tx)        

    }


    /// SetID sets ID of a transaction
    fn set_id(&mut self) -> Result<()> {
        let mut hasher = Sha256::new();
        let data = bincode::serialize(self)?;
        hasher.input(&data);
        self.id = hasher.result_str();
        Ok(())
    }

    
    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].txid.is_empty() && self.vin[0].vout == -1
    }


    pub fn sign(&mut self, private_key: &[u8], prev_TXs: HashMap<String, Transaction>) -> Result<()> {
        if self.is_coinbase() {
            return Ok(())
        }
        
        for vin in &self.vin {
            if prev_TXs.get(&vin.txid).unwrap().id.is_empty() {
                return Err(format_err!("ERROR: Previous transaction is not correct!"));
            }
        }

        let mut tx_copy = self.trim_copy();

        for in_id in ..tx_copy.vin.len() {
            let prev_Tx = prev_TXs.get(&tx_copy.vin[in_id].txid).unwrap();
            tx_copy.vin[in_id].signature.clear();
            tx_copy.vin[in_id].pub_key = prev_Tx.vout[tx_copy.vin[in_id].vout as usize]
                .pub_key_hash
                .clone();
            tx_copy.id = tx_copy.hash()?;
            tx_copy.vin[in_id].pub_key = Vec::new();
            
        }

        Ok(())
    }

    fn trim_copy(&self) -> Transaction {
        let mut vin = Vec::new();
        let mut vout = Vec::new();

        for v in &self.vin {
            vin.push(
                TXInput {
                    txid: v.txid.clone(),
                    vout: v.vout.clone(),
                    signature: Vec::new(),
                    pub_key: Vec::new(),
                }
            );

        }

        for v in &self.vout {
            vout.push(TXOutput {
                value: v.value,
                pub_key_hash: v.pub_key_hash.clone(),
            });
        }

        Transaction {
            id: self.id.clone(),
            vin,
            vout
        }

    }

}


