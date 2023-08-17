extern crate midir;

use std::{
    env,
    error::Error,
    io::{stdin, stdout, Write},
    sync::mpsc,
};

use anyhow::Result;
use midir::{Ignore, MidiInput};
use obws::{requests::scene_items::Id, Client};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env::set_var("RUST_LOG", "obws=debug");
    tracing_subscriber::fmt::init();
    let client = Client::connect("localhost", 4455, Some("password")).await?;
    let scene = "Scene";
    let client_inputs = client.inputs();
    let sis = client.scene_items();
    let si = client.scene_items().list("Scene").await.unwrap();
    println!("{si:#?}");
    let sii = client
        .scene_items()
        .id(Id {
            scene: "Scene",
            source: "test_text",
            search_offset: None,
        })
        .await?;
    let x = client.scene_items().index("Scene", sii).await.unwrap();

    println!("{x:#?}");
    let scene = &si[x as usize];
    println!("{scene:#?}");
    // scene.
    println!("{sii:#?}");
    let ins = client_inputs.list(None).await?;
    println!("{ins:#?}");
    // client_inputs.toggle_mute("test_text").await?; // mute my mic
    // client_inputs
    match run(client) {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
    Ok(())
}

fn run(client: Client) -> Result<(), Box<dyn Error>> {
    // Inside your run function, create a channel:
    let (tx, mut rx) = mpsc::channel();
    let mut tog_state = false;
    let mut input = String::new();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            println!(
                "Choosing the only available input port: {}",
                midi_in.port_name(&in_ports[0]).unwrap()
            );
            &in_ports[0]
        }
        _ => {
            println!("\nAvailable input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                println!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            print!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            in_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid input port selected")?
        }
    };

    println!("\nOpening connection");
    let in_port_name = midi_in.port_name(in_port)?;

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of
    // the scope

    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |stamp, message, _| {
            let len = message.len();
            if len != 1 {
                println!("{}: {:?} (len = {})", stamp, message, len)
            }
            match message {
                [144, 36, _] => {
                    println!(" pressed");
                    tx.send("toggle_mute").unwrap(); // This is a synchronous send
                }
                [128, 36, 0] => {
                    println!("released");
                    tx.send("toggle_mute").unwrap(); // This is a synchronous send
                }
                [144, 37, _] => {
                    tx.send("toggle_text").unwrap(); // This is a synchronous send
                }
                _ => (),
            }
        },
        (),
    )?;
    println!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name
    );

    tokio::task::spawn_blocking(move || {
        for msg in rx.iter() {
            match msg {
                "toggle_mute" => {
                    tokio::task::block_in_place(|| {
                        let toggle_mute_future = async {
                            client.inputs().toggle_mute("Mic/Aux").await.unwrap();
                        };
                        tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(toggle_mute_future);
                    });
                }
                "toggle_text" => {
                    tokio::task::block_in_place(|| {
                        tog_state = !tog_state;
                        let toggle_mute_future = async {
                            let sii = client
                                .scene_items()
                                .id(Id {
                                    scene: "Scene",
                                    source: "test_text",
                                    search_offset: None,
                                })
                                .await
                                .unwrap();
                            let se = obws::requests::scene_items::SetEnabled {
                                scene: "Scene",
                                item_id: sii,
                                enabled: tog_state,
                            };
                            client.scene_items().set_enabled(se).await.unwrap();
                        };
                        tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(toggle_mute_future);
                    });
                }
                _ => {}
            }
        }
    });

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");
    Ok(())
}
