pub mod initial_block_download;
pub mod handshake;
pub mod block_downloader;
pub mod data_handler;
pub mod utxo_set;

use crate::blocks::transaction::TxOut;
use crate::blocks::blockchain::*;
use std::collections::HashMap;
use crate::messages::*;
use crate::config::*;
use crate::log::*;
//use crate::messages::utils::MessageError;
use std::{
    io::{Read, Write},
    net::{SocketAddr, ToSocketAddrs, TcpStream},
};


const MESSAGE_HEADER_SIZE: usize = 24;
const DNS_ADDRESS: &str = "seed.testnet.bitcoin.sprovoost.nl";


/// Struct that represents the errors that can occur in the Node
#[derive(Debug)]
pub enum NodeError {
    ErrorConnectingToPeer,
    ErrorSendingMessageInHandshake,
    ErrorReceivingMessageInHandshake,
    ErrorReceivedUnknownMessage,
    ErrorInterpretingMessageCommandName,
    ErrorSendingMessageInIBD,
    ErrorIteratingStreams,
    ErrorReceivingHeadersMessageInIBD,
    ErrorReceivingMessageHeader,
    ErrorReceivingHeadersMessageHeaderInIBD,
    ErrorCreatingBlockDownloader,
    ErrorDownloadingBlockBundle,
    ErrorCreatingNode,
    ErrorSavingDataToDisk,
    ErrorLoadingDataFromDisk,
}

/// Struct that represents the bitcoin node
pub struct Node {
    version: i32,
    sender_address: SocketAddr,
    tcp_streams: Vec<TcpStream>,
    block_headers: Vec<BlockHeader>,
    blockchain: HashMap<[u8;32], Block>, //Vec<Block>, 
    utxo_set: HashMap<[u8;32], &'static TxOut>,
    logger: Logger,
}

impl Node {

    /// It creates and returns a Node with the default values
    fn _new(version: i32, local_host: [u8; 4], local_port: u16, logger: Logger) -> Node {
        Node {
            version,
            sender_address: SocketAddr::from((local_host, local_port)),
            tcp_streams: Vec::new(),
            block_headers: Vec::new(),
            blockchain: HashMap::new(),
            utxo_set: HashMap::new(),
            logger,
        }
    }

    /// Node constructor, it creates a new node and performs the handshake with the sockets obtained
    /// by doing peer_discovery. If the handshake is successful, it adds the socket to the
    /// tcp_streams vector. Returns the node
    pub fn new(config: Config) -> Result<Node, NodeError> {
        let logger = match Logger::from_path(config.log_path.as_str()){
            Ok(logger) => logger,
            Err(_) => return Err(NodeError::ErrorCreatingNode),
        };
        let mut node = Node::_new(config.version, config.local_host, config.local_port, logger);
        let address_vector = node.peer_discovery(DNS_ADDRESS, config.dns_port);
        
        for addr in address_vector {
            if let Ok(tcp_stream) = node.handshake(addr) {
                node.tcp_streams.push(tcp_stream);
            }
        }

        Ok(node)
    }

    /// Receives a dns address as a String and returns a Vector that contains all the addresses
    /// returned by the dns. If an error occured (for example, the dns address is not valid), it
    /// returns an empty Vector.
    /// The socket address requires a dns and a DNS_PORT, which is set to 53 by default
    fn peer_discovery(&self, dns: &str, dns_port: u16) -> Vec<SocketAddr> {
        let mut socket_address_vector = Vec::new();

        if let Ok(address_iter) = (dns, dns_port).to_socket_addrs() {
            for address in address_iter {
                socket_address_vector.push(address);
            }
        }
        socket_address_vector
    }

    /// Returns a reference to the tcp_streams vector
    pub fn get_tcp_streams(&self) -> &Vec<TcpStream> {
        &self.tcp_streams
    }

    pub fn get_blockchain(&self) -> &HashMap<[u8; 32], Block>{
        &self.blockchain
    }
            
    ///Generic receive message function, receives a header and its payload, and calls the corresponding handler. Returns the command name in the received header
    fn receive_message (&mut self, stream_index: usize) -> Result<String, NodeError>{
        let mut stream = &self.tcp_streams[stream_index];
        let block_headers_msg_h = receive_message_header(&mut stream)?;
        println!("\n{}", block_headers_msg_h.get_command_name());
        
        let mut msg_bytes = vec![0; block_headers_msg_h.get_payload_size() as usize];
        match stream.read_exact(&mut msg_bytes) {
            Err(_) => return Err(NodeError::ErrorReceivingHeadersMessageInIBD),
            Ok(_) => {}
        }

        match block_headers_msg_h.get_command_name().as_str(){
            "ping\0\0\0\0\0\0\0\0" => self.handle_ping(stream_index, &block_headers_msg_h, msg_bytes),
            "inv\0\0\0\0\0\0\0\0\0" => {},
            "block\0\0\0\0\0\0" => {},
            "headers\0\0\0\0\0" => self.handle_block_headers_message(msg_bytes, stream_index)?,
            //"block\0\0\0\0\0\0\0" => self.handle_block_message(msg_bytes)?,
            _ => {},
        };
        Ok(block_headers_msg_h.get_command_name())
    }

    fn handle_ping(&self, stream_index: usize, header_message: &HeaderMessage, nonce: Vec<u8>){
        if nonce.len() != 8{
            return
        }
        let mut stream = &self.tcp_streams[stream_index];

        let mut pong_bytes = header_message.to_bytes();
        pong_bytes.extend(nonce);
        pong_bytes[5] = b'o';
        //p manejar desp
        stream.write(&pong_bytes);
    }
}

///Reads from the stream MESAGE_HEADER_SIZE bytes and returns a HeaderMessage interpreting those bytes acording to bitcoin protocol.
/// On error returns ErrorReceivingMessage
pub fn receive_message_header<T: Read + Write>(stream: &mut T,) -> Result<HeaderMessage, NodeError> {
    let mut header_bytes = [0; MESSAGE_HEADER_SIZE];
    if let Err(_) = stream.read_exact(&mut header_bytes){
        return Err(NodeError::ErrorReceivingMessageHeader);
    };
    match HeaderMessage::from_bytes(&mut header_bytes) {
        Ok(header_message) => Ok(header_message),
        Err(_) => Err(NodeError::ErrorReceivingMessageHeader),
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_tcp_stream::*;


    const VERSION: i32 = 70015;
    const DNS_PORT: u16 = 18333;
    const LOCAL_HOST: [u8; 4] = [127, 0, 0, 1];
    const LOCAL_PORT: u16 = 1001; 


    #[test]
    fn peer_discovery_test_1_fails_when_receiving_invalid_dns_address() {
        let logger = Logger::from_path("test_log.txt").unwrap();
        let node = Node::_new(VERSION, LOCAL_HOST, LOCAL_PORT, logger);
        let address_vector = node.peer_discovery("does_not_exist", DNS_PORT);

        assert!(address_vector.is_empty());
    }

    #[test]
    fn peer_discovery_test_2_returns_ip_vector_when_receiving_valid_dns() {
        let logger = Logger::from_path("test_log.txt").unwrap();
        let node = Node::_new(VERSION, LOCAL_HOST, LOCAL_PORT, logger);
        let address_vector = node.peer_discovery(DNS_ADDRESS, DNS_PORT);

        assert!(!address_vector.is_empty());
    }

    #[test]
    fn node_test_1_receive_header_message() -> Result<(), NodeError> {
        let mut stream = MockTcpStream::new();

        let logger = Logger::from_path("test_log.txt").unwrap();
        let node = Node::_new(VERSION, LOCAL_HOST, LOCAL_PORT, logger);

        let expected_hm =
            HeaderMessage::new("test message", &Vec::from("test".as_bytes())).unwrap();
        stream.read_buffer = expected_hm.to_bytes();

        let received_hm = receive_message_header(&mut stream)?;

        assert_eq!(received_hm, expected_hm);
        Ok(())
    }
}