use crate::utils::NodeError;
use crate::utils::ui_communication_protocol::{
    UIToWalletCommunication as UIRequest,
    WalletToUICommunication as UIResponse,
    WalletInfo,
};
use glib::{Sender as GlibSender};
use crate::{blocks::transaction::*, utils::WalletError};
use crate::node::Node;
use super::Wallet;


impl Wallet{
    pub fn handle_ui_request(mut self, node: &mut Node, request: UIRequest, sender_to_ui: &GlibSender<UIResponse>, program_running: &mut bool)-> Result<Wallet, WalletError>{
        let ui_response = match request{
            UIRequest::ChangeWallet(priv_key_string) => match self.handle_change_wallet(node, priv_key_string){
                Ok(wallet) => return Ok(wallet),
                Err(wallet_error) => Err(wallet_error),
            },
            UIRequest::CreateTx(amount, fee, address) => self.handle_create_tx(node, amount, fee, address),
            UIRequest::Update => Ok(self.handle_update()),
            UIRequest::LastBlockInfo => self.handle_last_block_info(node),
            UIRequest::NextBlockInfo => self.handle_get_block_info(node, self.current_block - 1),
            UIRequest::PrevBlockInfo => self.handle_get_block_info(node, self.current_block + 1),
            //UIRequest::MerkleProof(tx) => todo!(),
            UIRequest::EndOfProgram => {
                *program_running = false;
                return Ok(self);
            },
        };

        match ui_response{
            Ok(ui_response) => sender_to_ui.send(ui_response).map_err(|_| WalletError::ErrorSendingToUI)?,
            Err(wallet_error) => sender_to_ui.send(UIResponse::WalletError(wallet_error)).map_err(|_| WalletError::ErrorSendingToUI)?,
        }
        Ok(self)
    }

    pub fn handle_get_block_info(&mut self, node: &Node, block_number: usize) -> Result<UIResponse, WalletError>{
        let block_info = match node.get_block_info_from(block_number){
            Ok(block_info) => block_info,
            Err(error) => {
                let wallet_error = match error {
                    NodeError::ErrorFindingBlock => WalletError::ErrorFindingBlock,   
                    _ => WalletError::ErrorGettingBlockInfo,
                };
                return Err(wallet_error);
            }
        };
        
        self.current_block = block_info.block_number;
        
        Ok(UIResponse::BlockInfo(block_info))
    }
    
    pub fn handle_last_block_info(&mut self, node: &Node) -> Result<UIResponse, WalletError>{
        let block_info = match node.get_last_block_info(){
            Ok(block_info) => block_info,
            Err(error) => {
                let wallet_error = match error {
                    NodeError::ErrorFindingBlock => WalletError::ErrorFindingBlock,   
                    _ => WalletError::ErrorGettingBlockInfo,
                };
                return Err(wallet_error);
            }
        };
        
        self.current_block = block_info.block_number;
        
        Ok(UIResponse::BlockInfo(block_info))
    }

    pub fn handle_update(&self) -> UIResponse{
        let wallet_info = WalletInfo::from(self);

        UIResponse::WalletInfo(wallet_info)
    }

    pub fn handle_change_wallet(&self, node: &mut Node, priv_key_string: String) -> Result<Wallet, WalletError>{
        let mut new_wallet = Wallet::from(priv_key_string)?;
        node.set_wallet(&mut new_wallet).map_err(|_| WalletError::ErrorSetingWallet)?;
        //new_wallet = new_wallet.handle_ui_request(node, UIRequest::Update, sender_to_ui, &mut true)?;
        Ok(new_wallet)
    }

    fn handle_create_tx(&self, node: &mut Node, amount: i64, fee: i64, receiver_address: String)-> Result<UIResponse, WalletError>{
        let address_bytes = bs58::decode(receiver_address).into_vec().map_err(|_| WalletError::ErrorSendingTx)?;
        let mut address: [u8;25] = [0;25];
        address.copy_from_slice(&address_bytes);
        self.create_transaction(node, amount, fee, address)?;
        
        Ok(UIResponse::TxSent)
    }
}