use std::path::PathBuf;
use std::process::{Command, Stdio, ChildStdin, ChildStdout};
use std::io::{BufReader, Write, BufRead, Read};
use vampirc_uci::{ByteVecUciMessage, UciMessage, parse_one, UciFen, UciSearchControl, UciTimeControl, UciInfoAttribute};
use chess::Game;
use std::thread;
use std::time::Duration;

struct Uci {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Uci {
    /// Starts an engine initializing it by taking a Command with all
    /// appropriate arguments passed for UCI
    pub fn start_engine(engine :&mut Command) -> Self {
        // create a child process
        let child = engine.stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .expect("Error starting engine");

        let mut stdin = child.stdin.unwrap();
        let mut stdout = BufReader::new(child.stdout.unwrap());

        let mut uci = Uci {
            stdin,
            stdout
        };

        // init with the UCI message
        uci.send_msg(UciMessage::Uci);

        // we manually read because a lot of engines send non-UCI at first
        let mut msg_buffer = String::new();

        uci.stdout.read_line(&mut msg_buffer).expect("Error reading");

        while msg_buffer.find("id ").is_none() {
            msg_buffer.clear();
            uci.stdout.read_line(&mut msg_buffer).expect("Error reading");
        }

        // found the first id line
        let start = msg_buffer.find("id ").unwrap();
        let mut message = parse_one(&msg_buffer.as_str()[start..]);

        loop {
            println!("MSG: {:?}", message);

            // go until we get the OK
            if let UciMessage::UciOk = message {
                break
            }

            // keep reading messages
            message = uci.recv_msg();
        }

        // TODO: add option setting here

        // check to see if it's ready
        uci.send_msg(UciMessage::IsReady);
        message = uci.recv_msg();

        println!("MSG: {:?}", message);

        if UciMessage::ReadyOk != message {
            panic!("Error setting up engine");
        }

        // let the engine we're staring a new game
        uci.send_msg(UciMessage::UciNewGame);

        // also tell it to use analysis mode
        uci.send_msg(UciMessage::SetOption { name: "UCI_AnalyseMode".to_string(), value: Some("true".to_string()) });

        // check to see if it's ready
        uci.send_msg(UciMessage::IsReady);
        message = uci.recv_msg();

        if let UciMessage::ReadyOk = message {
            uci
        } else {
            panic!("Error setting up engine");
        }
    }

    fn send_msg(&mut self, message :UciMessage) {
        self.stdin.write_all(ByteVecUciMessage::from(message).as_ref()).expect("Error writing");
        self.stdin.flush().expect("Error flushing");
    }

    fn recv_msg(&mut self) -> UciMessage {
        let mut buff = String::new();

        self.stdout.read_line(&mut buff).expect("Error reading");
        parse_one(buff.as_str())
    }

    pub fn analyze(&mut self, game :&Game) {
        println!("CUR POS: {}", game.current_position().to_string());

        // set the position
        self.send_msg(UciMessage::Position {
            startpos: false,
            fen: Some(UciFen(game.current_position().to_string())),
            moves: vec![]
        });

        // tell the engine to start processing
        // self.send_msg(UciMessage::Go {
        //     time_control: None,
        //     search_control: Some(UciSearchControl {
        //         search_moves: vec![],
        //         mate: None,
        //         depth: Some(3),
        //         nodes: None
        //     })
        // });

        self.send_msg(UciMessage::Go {
            time_control: Some(UciTimeControl::Infinite),
            search_control: None
        });

        // let it think for a while
        thread::sleep(Duration::from_secs(1));

        // tell it to stop thinking
        self.send_msg(UciMessage::Stop);

        // read everything it sent back
        loop {
            let message = self.recv_msg();
            // println!("MSG: {:?}", message);

            match message {
                UciMessage::Info(attrs) => {
                    let mut info = String::new();

                    for attr in attrs {
                        match attr {
                            UciInfoAttribute::Depth(d) => { info.push_str(&format!("DEPTH: {}", d)); },
                            UciInfoAttribute::Score { cp, .. } => { info.push_str(&format!(" SCORE: {}", cp.unwrap())); },
                            UciInfoAttribute::Pv(moves) => { info.push_str(&format!(" {}", moves.into_iter().map(|m| m.to_string()).collect::<Vec<_>>().join(","))); }
                            UciInfoAttribute::CurrMove(chess_move) => { info.push_str(&chess_move.to_string()); },
                            _ => ()
                        }
                    }

                    println!("{}", info);
                },
                UciMessage::BestMove { best_move, ponder } => {
                    println!("BEST MOVE: {}", best_move);
                    if let Some(ponder) = ponder {
                        println!("PONDER: {}", ponder);
                    }
                    break
                }
                _ => {
                    panic!("Unexpected message: {:?}", message)
                }
            }
        }
    }
}


#[cfg(test)]
mod uci_tests {
    use std::process::Command;
    use chess::Game;
    use crate::uci::Uci;

    // #[test]
    // fn start_gnuchess_test() {
    //     let mut cmd = Command::new("/usr/games/gnuchess");
    //
    //     let uci = Uci::start_engine(cmd.arg("-u"));
    // }

    #[test]
    fn start_stockfish_test() {
        let mut cmd = Command::new("/usr/games/stockfish");

        let uci = Uci::start_engine(&mut cmd);
    }

    #[test]
    fn start_ethereal_test() {
        let mut cmd = Command::new("/usr/games/ethereal-chess");

        let uci = Uci::start_engine(&mut cmd);
    }

    #[test]
    fn analyze_test() {
        let mut cmd = Command::new("/usr/games/ethereal-chess");
        let mut uci = Uci::start_engine(&mut cmd);
        let game = Game::new();

        uci.analyze(&game);
    }
}