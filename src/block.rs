use std::time::SystemTime;
use crypto::{digest::Digest, sha2::Sha256};
use log::info;
use serde::{Deserialize, Serialize};

use crate::{error::Result, transaction::Transaction};

pub const TARGET_HEXT: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    timestamp: u128,
    transactions: Vec<Transaction>,
    prev_block_hash: String,
    hash: String,
    height: usize,
    nonce: i32
}


impl Block {

    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub fn new_genesis_block(coinbase: Transaction) -> Block {
        Block::new_block(vec![coinbase], String::new(), 0).unwrap()
    }

    pub fn new_block(data: Vec<Transaction>, prev_block_hash: String, height: usize) -> Result<Block> {
        let timestamp: u128 = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap()
            .as_millis();

        let mut block = Block {
            timestamp,
            transactions: data,
            prev_block_hash,
            hash: String::new(),
            height,
            nonce: 0
        };

        block.run_proof_if_work()?;
        Ok(block)

    }

    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }

    fn run_proof_if_work(&mut self) -> Result<()> {

        info!("Mining the block!");
        
        while !self.validate().unwrap() {
            self.nonce += 1;
        }

        let data: Vec<u8> = self.preapre_hash_data().unwrap();

        let mut hasher = Sha256::new();

        hasher.input(&data[..]);
        self.hash = hasher.result_str();
        Ok(())

    }

    fn preapre_hash_data(&self) -> Result<Vec<u8>> {
        let content = (
            self.prev_block_hash.clone(),
            self.transactions.clone(),
            self.timestamp,
            TARGET_HEXT,
            self.nonce
        );

        let bytes = bincode::serialize(&content)?;
        Ok(bytes)

    }

    fn validate(&self) -> Result<bool> {
        let data = self.preapre_hash_data()?;
        let mut hasher = Sha256::new();

        hasher.input(&data[..]);

        let mut vec1: Vec<u8> = vec![];
        vec1.resize(TARGET_HEXT, '0' as u8);
        
        Ok(&hasher.result_str()[0..TARGET_HEXT] == String::from_utf8(vec1).unwrap())
    }

    pub fn get_prev_hash(&self) -> String {
        self.prev_block_hash.clone()
    }

}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::Blockchain;

    #[test]
    fn test_blockchain() {

        let mut b = Blockchain::new().unwrap();
        // b.add_block("data".to_string());
        // b.add_block("data2".to_string());
        // b.add_block("data3".to_string());
       
        for item in b.iter() {
            println!("Item: {:?}", item)
        }


    }

}