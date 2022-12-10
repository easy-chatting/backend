use std::collections::HashMap;
use crate::crypto::RoomId;
use crate::ClientId;
use anyhow::anyhow;
use base64ct::{Base64Url, Encoding};
use tokio::sync::mpsc;
use warp::ws::Message;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum RoomState {
    Open,
    Locked,
}

#[derive(Debug)]
pub struct Room {
    pub id: RoomId,
    pub invite_link: String,
    pub state: RoomState,
    // pub clients: Vec<ClientId>,
    pub clients: HashMap<ClientId, mpsc::UnboundedSender<Message>>
}

impl Room {
    pub fn new(id: RoomId, base_url: &str) -> Self {
        let mut buf = [0u8; 44];
        let base64_id =
            Base64Url::encode(&id, &mut buf).expect("Expected room ID to be encoded in Base64");
        let invite_link = format!("{}/join/{}", base_url, base64_id);

        Self {
            id,
            invite_link,
            state: RoomState::Open,
            clients: HashMap::with_capacity(4),
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, RoomState::Open)
    }

    pub fn remove_client(&mut self, client_id: &ClientId) -> bool {
        self.clients.remove(client_id).is_some()
    }

    // pub fn get_other_client_ids(&self, client_id: &ClientId) -> Vec<ClientId> {
    //     self.clients
    //         .iter()
    //         .filter_map(|id| if id == client_id { Some(*id) } else { None })
    //         .collect()
    // }
    
    pub fn get_other_clients(&self, client_id: &ClientId) -> Vec<mpsc::UnboundedSender<Message>> {
        self.clients
            .iter()
            .filter_map(|(other_client_id, other_client_tx)| {
                if other_client_id != client_id {
                    Some(other_client_tx.clone())
                } else {
                    None
                }
            }).collect()
    }
}

pub fn base64_to_room_id(input: &str) -> anyhow::Result<RoomId> {
    let mut buf = [0u8; 32];
    match Base64Url::decode(input.as_bytes(), &mut buf) {
        Ok(_) => Ok(buf),
        Err(err) => Err(anyhow!("Unable to decode Base64 string: {}", err)),
    }
}

#[cfg(test)]
mod tests {
    use crate::room::*;

    #[test]
    fn test_new_room() {
        let room_id = b"abcdefghijklmnopqrstuvwzyx123456";
        let base_url = "https://example.com";
        let room = Room::new(*room_id, base_url);

        assert_eq!(
            room.invite_link,
            "https://example.com/join/YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd6eXgxMjM0NTY="
        );
        assert_eq!(room.state, RoomState::Open);
    }

    #[test]
    fn test_base64() {
        let room_id = b"abcdefghijklmnopqrstuvwzyx123456";
        let decoded_base64 =
            base64_to_room_id("YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd6eXgxMjM0NTY=").unwrap();
        assert_eq!(room_id, &decoded_base64);
    }
}
