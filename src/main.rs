use std::{ collections::HashMap, io::{ Read, Write }, time::Duration };

use mio::{ net::{ TcpListener, TcpStream }, Events, Interest, Token };

use crate::io::poll::PollManager;

pub mod io;

const SERVER: Token = Token(0);

struct Client {
  stream: TcpStream,
  token: Token,
  read_buffer: Vec<u8>,
  write_buffer: Vec<u8>,
}

fn would_block(e: &std::io::Error) -> bool {
  e.kind() == std::io::ErrorKind::WouldBlock
}

fn interrupted(e: &std::io::Error) -> bool {
  e.kind() == std::io::ErrorKind::Interrupted
}

fn next(current: &mut Token) -> Token {
  let next = current.0;
  current.0 += 1;
  Token(next)
}

fn main() -> Result<(), std::io::Error> {
  let mut poll_manager = PollManager::new()?;

  let port = std::env::args().nth(1).unwrap_or("8080".to_string());

  let addr = format!("0.0.0.0:{}", port).parse().unwrap();
  let mut server_socket = TcpListener::bind(addr)?;
  poll_manager.add(&mut server_socket, SERVER, Interest::READABLE)?;

  let mut unique_token = Token(SERVER.0 + 1);
  let mut clients: HashMap<Token, Client> = HashMap::new();
  let mut events = Events::with_capacity(128);

  println!("Server listening on: {}", addr);
  loop {
    if let Err(e) = poll_manager.poll(&mut events, Some(Duration::from_millis(500))) {
      if interrupted(&e) {
        continue;
      }
      return Err(e);
    }
    for event in events.iter() {
      match event.token() {
        SERVER =>
          loop {
            println!("New client connection");
            let (stream, addr) = match server_socket.accept() {
              Ok((stream, addr)) => (stream, addr),
              Err(e) if would_block(&e) => {
                break;
              }
              Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
                continue;
              }
            };
            let token = next(&mut unique_token);
            let client = Client {
              stream,
              token,
              read_buffer: Vec::with_capacity(1024),
              write_buffer: Vec::with_capacity(1024),
            };
            clients.insert(token, client);
            poll_manager.add(
              &mut clients.get_mut(&token).unwrap().stream,
              token,
              Interest::READABLE
            )?;
            println!("Accepted connection from: {}", addr);
            #[cfg(windows)]
            poll_manager.update(&mut server_socket, SERVER, Interest::READABLE)?;
          }
        token => {
          let mut remove_client = false;
          let mut buffer = [0; 1024];
          let client = match clients.get_mut(&token) {
            Some(client) => client,
            None => {
              eprintln!("Received event for unknown token: {:?}", token);
              continue;
            }
          };
          if event.is_readable() {
            remove_client = match client.stream.read(&mut buffer) {
              Ok(0) => true,
              Ok(n) => {
                client.read_buffer.extend_from_slice(&buffer[..n]);
                println!("Read {} bytes from client", n);
                if let Ok(str) = std::str::from_utf8(&client.read_buffer) {
                  println!("Data: {:?}", str);
                } else {
                  println!("Data (non-utf8): {:?}", &client.read_buffer);
                }
                const CRLF: &[u8] = b"\r\n\r\n";
                if
                  let Some(pos) = client.read_buffer
                    .windows(CRLF.len())
                    .position(|window| window == CRLF)
                {

                  let http_body = "Hello, World!";
                  let http_response = format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    http_body.len(),
                    http_body
                  );
                  client.write_buffer.extend_from_slice(http_response.as_bytes());
                  poll_manager.update(&mut client.stream, client.token, Interest::WRITABLE)?;
                  client.read_buffer = client.read_buffer.split_off(pos + CRLF.len());
                } else {
                  #[cfg(windows)]
                  poll_manager.update(&mut client.stream, client.token, Interest::READABLE)?;
                }
                false
              }
              Err(e) => {
                eprintln!("Failed to read from client: {}", e);
                true
              }
            };
          }
          if event.is_writable() {
            match client.stream.write(&client.write_buffer) {
              Ok(n) => {
                println!("Wrote {} bytes to client", n);
                let end_slice = &mut client.write_buffer[n..];
                client.write_buffer = end_slice.to_vec();
              }
              Err(e) if would_block(&e) => {}
              Err(e) if interrupted(&e) => {}
              Err(e) => eprintln!("Failed to write to client: {}", e),
            }
            remove_client = true;
          }
          if remove_client {
            poll_manager.remove(&mut client.stream)?;
            clients.remove(&token);
          }
        }
      }
    }
  }
}
