mod ftp;
mod parser;

use std::path::PathBuf;
use std::str;
use std::{net::SocketAddr, path::Path};

use miette::*;
use num_integer::Integer;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tracing::*;

use crate::ftp::*;
use crate::parser::*;

/// Handles the data connection for the FTP server
///
/// This function is spawned as a new task for each data connection
#[instrument]
async fn handle_data(socket: (TcpStream, SocketAddr), path: PathBuf) -> Result<()> {
    let (mut stream, addr) = socket;
    info!("New data connection from {}", addr);
    let mut buf: Vec<u8> = vec![];
    let _ = stream.read_to_end(&mut buf).await.into_diagnostic()?;
    let content = str::from_utf8(&buf);
    let mut file = File::create(path.join("tortilla.txt"))
        .await
        .into_diagnostic()?;
    file.write_all(&buf).await.into_diagnostic()?;
    debug!(?content);
    Ok(())
}

/// Handles the client connection for the FTP server
///
/// This function is spawned as a new task for each client connection
///
/// ## Usage
///
/// ```
/// use std::net::SocketAddr;
/// use tokio::net::TcpStream;
///
/// use ftp_server::handle_client;
///
/// let addr = SocketAddr::from(([127, 0, 0, 1], 2121));
/// let stream = TcpStream::connect(addr).await.unwrap();
/// loop {
///     let socket = stream.accept().await.unwrap();
///     tokio::spawn(async move { handle_client(socket).await });
/// }
/// ```
#[instrument]
async fn handle_client(socket: (TcpStream, SocketAddr)) -> Result<()> {
    let (mut stream, addr) = socket;
    let (mut read_stream, mut write_stream) = stream.split();
    let mut reader = BufReader::new(&mut read_stream);
    let mut cwd = Path::new("./").to_path_buf();
    // let mut writer = BufWriter::new(&mut write_stream);
    info!("New client connection from {}", addr);
    // ftp authorization logic goes here
    write_stream.write_all(b"220\n").await.into_diagnostic()?;
    let mut buf = vec![];
    loop {
        let _ = reader.read_until(b'\n', &mut buf).await.into_diagnostic()?;
        let input = str::from_utf8(&buf).into_diagnostic()?.trim_end();
        debug!("Reading {:?} from stream", input);
        let (_, (cmd, args)) = cmd_parser(input).unwrap();
        info!("Received {:?} command with args: {:?}", cmd, args);
        match cmd {
            "USER" => {
                // Return OK Authorized for now
                write_stream.write_all(b"200\n").await.into_diagnostic()?;
            }
            "SYST" => {
                write_stream.write_all(b"200\n").await.into_diagnostic()?;
            }
            "PORT" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "PASV" => {
                let data_addr = SocketAddr::from(([127, 0, 0, 1], 2222));
                let data_port = data_addr.port();
                let (port_high, port_low) = data_port.div_rem(&256);
                let data_listener = TcpListener::bind(data_addr)
                    .await
                    .unwrap_or_else(|_| panic!("Could not bind to address {}", data_addr));
                write_stream
                    .write_all(
                        StatusCode::EnteringPassiveMode {
                            port_high,
                            port_low,
                        }
                        .to_string()
                        .as_bytes(),
                    )
                    .await
                    .into_diagnostic()?;
                let cwd = cwd.clone();
                let data_socket = data_listener
                    .accept()
                    .await
                    .expect("Error accepting connection to data_socket");
                tokio::spawn(async move { handle_data(data_socket, cwd).await });
            }
            "STOR" => {
                write_stream
                    .write_all(StatusCode::DataOpenTransfer.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "LPRT" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "RETR" => {}
            "NOOP" => {
                stream.shutdown().await.into_diagnostic()?;
                break;
            }
            "QUIT" => {
                stream.shutdown().await.into_diagnostic()?;
                break;
            }
            "TYPE" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "MODE" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "STRU" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "CWD" => {
                cwd = cwd.join(args[0]);
                write_stream
                    .write_all(StatusCode::FileActionOk.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            "PWD" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            // this command is a synonym for CWD
            "CDUP" => {
                write_stream
                    .write_all(StatusCode::CmdNotImplemented.to_string().as_bytes())
                    .await
                    .into_diagnostic()?;
            }
            &_ => {}
        }
        debug!("Clearing buffer");
        buf.clear();
    }
    info!("Client {} disconnected", addr);
    // stream.shutdown().await?;
    Ok(())
}

#[tokio::main]
#[instrument]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let addr = SocketAddr::from(([127, 0, 0, 1], 2121));
    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|_| panic!("Could not bind to address {}", addr));
    info!("Listening to addr {}", addr);
    loop {
        let socket = listener
            .accept()
            .await
            .expect("Error accepting connection to socket");
        tokio::spawn(async move { handle_client(socket).await });
    }
}
