mod crypto;
mod msg;
mod room;

use crate::crypto::{RoomId, Secret};
use crate::room::base64_to_room_id;
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::{Filter, Rejection};

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(0);

type ClientId = usize;
type Room = Arc<RwLock<room::Room>>;
type Rooms = Arc<RwLock<HashMap<RoomId, Room>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let base_url = match std::env::var("BASE_URL") {
        Ok(value) => value,
        Err(err) => {
            panic!("Couldn't get environment variable \"BASE_URL\": {}", err);
        }
    };

    let base_url = Arc::new(base_url);
    let base_url = warp::any().map(move || base_url.clone());

    let secret = crypto::generate_secret();
    let secret = Arc::new(secret);
    let secret = warp::any().map(move || secret.clone());

    let rooms = Rooms::default();
    let rooms = warp::any().map(move || rooms.clone());

    let create_room = warp::post()
        .and(warp::path("create"))
        .and(rooms.clone())
        .and(secret)
        .and(base_url)
        .and_then(create_room);

    let join_room = warp::path("join")
        .and(warp::path::param::<String>())
        .and(rooms)
        .and(warp::ws())
        .map(|room_id, rooms, ws: warp::ws::Ws| {
            ws.on_upgrade(move |socket| client_connected(socket, room_id, rooms))
        });

    let index = warp::path::end().map(|| warp::reply::html("no"));
    let routes = index.or(create_room).or(join_room);

    // TODO: graceful shutdown in response to signals (SIGTERM, SIGKILL)
    let server = warp::serve(routes);
    server.run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

async fn create_room(
    rooms: Rooms,
    secret: Arc<Secret>,
    base_url: Arc<String>,
) -> Result<impl warp::Reply, Rejection> {
    let room_id = match crypto::generate_room_identifier(&secret) {
        Ok(room_id) => room_id,
        Err(err) => {
            eprintln!("{}", err);
            // TODO: return something meaningful
            return Err(warp::reject::reject());
        }
    };

    let room = room::Room::new(room_id, &base_url);
    let room_invite_link = room.invite_link.clone();

    let room = Arc::new(RwLock::new(room));
    rooms.write().await.insert(room_id, room);

    Ok(warp::reply::json(&room_invite_link))
}

async fn client_connected(ws: WebSocket, room_id: String, rooms: Rooms) {
    let room_id = match base64_to_room_id(&room_id) {
        Ok(room_id) => room_id,
        Err(err) => {
            tracing::error!("Invalid Base64 string: \"{}\" {}", room_id, err);
            return;
        }
    };

    // get the room we want to join
    let room = {
        let rooms = rooms.read().await;
        let room = rooms.get(&room_id);
        match room {
            Some(room) => room.clone(),
            None => {
                tracing::error!("Client tried to join a room that doesn't exist");
                return;
            }
        }
    };

    {
        let room = room.read().await;
        if !room.is_open() {
            tracing::error!("Client tried to join a room that isn't open");
            return;
        }
    }

    // get a client identifier
    let client_id = NEXT_USER_ID.fetch_add(1, Ordering::Release);

    // split socket into sender and receiver of messages
    let (mut client_ws_tx, mut client_ws_rx) = ws.split();

    // use an unbounded channel to handle buffering and flushing of messages
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    // task for sending messages
    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            client_ws_tx
                .send(message)
                .unwrap_or_else(|e| eprintln!("websocket send error: {}", e))
                .await;
        }
    });

    // add the client to the room and get the room channel
    {
        let mut room = room.write().await;
        room.clients.insert(client_id, tx.clone());
    };

    // accept incoming messages from the sender sink
    while let Some(result) = client_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        };

        client_message(&tx, &client_id, msg, &room).await;
    }

    // the client is disconnected and client_ws_rx stopped receiving messages
    client_disconnected(&client_id, room).await;
}

async fn client_disconnected(client_id: &ClientId, room: Room) {
    let mut room = room.write().await;
    if !room.remove_client(client_id) {
        tracing::error!(
            "Unable to remove client from room because the client isn't connected to it!"
        );
    }
}

async fn client_message(
    client_tx: &mpsc::UnboundedSender<Message>,
    client_id: &ClientId,
    message: Message,
    room: &Room,
) {
    // don't have to manually respond to ping and close messages
    // only accept binary messages, text message means bad client
    if !message.is_binary() {
        return;
    }

    // TODO: message processing
    {
        let room = room.read().await;

        for other_client_rx in room.get_other_clients(client_id) {
            match other_client_rx.send(message.clone()) {
                Ok(_) => {}
                Err(err) => {
                    tracing::error!("Error sending message: {}", err);
                    continue;
                }
            }
        }
    }
}
