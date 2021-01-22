use std::process::{Command, Stdio, ChildStdin, ChildStdout};
use std::io::{BufReader, Write, BufRead};
use std::thread;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Mutex, Arc};

use log::{debug};
use vampirc_uci::{ByteVecUciMessage, UciMessage, parse_one, UciFen, UciSearchControl, UciTimeControl, UciInfoAttribute};
use chess::{Game, ChessMove};

#[derive(Clone, Debug)]
pub enum Analysis {
    PossibleMove(PossibleMove),
    BestMove(ChessMove)
}

/// This is a candidate move given the depth
#[derive(Clone, Default, Debug)]
pub struct PossibleMove {
    depth: u8,
    score: i32,
    moves: Vec<ChessMove>
}

#[derive(Debug, Clone)]
pub struct Uci {
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
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

        // init with the UCI message
        Self::send_msg(&mut stdin, UciMessage::Uci);

        // we manually read because a lot of engines send non-UCI at first
        let mut msg_buffer = String::new();

        stdout.read_line(&mut msg_buffer).expect("Error reading");

        while msg_buffer.find("id ").is_none() {
            msg_buffer.clear();
            stdout.read_line(&mut msg_buffer).expect("Error reading");
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
            message = Self::recv_msg(&mut stdout) ;
        }

        // TODO: add option setting here

        // check to see if it's ready
        Self::send_msg(&mut stdin, UciMessage::IsReady);
        message = Self::recv_msg(&mut stdout) ;

        println!("MSG: {:?}", message);

        if UciMessage::ReadyOk != message {
            panic!("Error setting up engine");
        }

        // let the engine we're staring a new game
        Self::send_msg(&mut stdin, UciMessage::UciNewGame);

        // also tell it to use analysis mode
        Self::send_msg(&mut stdin, UciMessage::SetOption { name: "UCI_AnalyseMode".to_string(), value: Some("true".to_string()) });

        // check to see if it's ready
        Self::send_msg(&mut stdin, UciMessage::IsReady);
        message = Self::recv_msg(&mut stdout) ;

        if let UciMessage::ReadyOk = message {
            Uci {
                stdin: Arc::new(Mutex::new(stdin)),
                stdout: Arc::new(Mutex::new(stdout))
            }
        } else {
            panic!("Error setting up engine");
        }
    }

    fn send_msg(stdin :&mut ChildStdin, message :UciMessage) {
        stdin.write_all(ByteVecUciMessage::from(message).as_ref()).expect("Error writing");
        stdin.flush().expect("Error flushing");
    }

    fn recv_msg(stdout: &mut BufReader<ChildStdout>) -> UciMessage {
        let mut buff = String::new();

        stdout.read_line(&mut buff).expect("Error reading");
        parse_one(buff.as_str())
    }

    /// Given a game, analyze that game to the given depth
    /// A Receiver of Analysis structs is returned
    /// When the depth is reached (None for infinite), or the Receiver is dropped,
    /// the engine will stop its analysis
    pub fn analyze(&mut self, game :&Game, depth :Option<u8>) -> Receiver<Analysis> {
        println!("CUR POS: {}", game.current_position().to_string());

        { // scope our lock
            let mut stdin = self.stdin.lock().unwrap();

            // set the position
            Self::send_msg(&mut stdin, UciMessage::Position {
                startpos: false,
                fen: Some(UciFen(game.current_position().to_string())),
                moves: vec![]
            });

            // tell the engine to start processing
            if depth.is_some() {
                Self::send_msg(&mut stdin, UciMessage::Go {
                    time_control: None,
                    search_control: Some(UciSearchControl {
                        search_moves: vec![],
                        mate: None,
                        depth: depth,
                        nodes: None
                    })
                });
            } else {
                Self::send_msg(&mut stdin, UciMessage::Go {
                    time_control: Some(UciTimeControl::Infinite),
                    search_control: None
                });
            }
        }

        // clone STDIN & STDOUT
        let stdin_clone = self.stdin.clone();
        let stdout_clone = self.stdout.clone();

        // create a channel for sending back the analysis
        let (tx, rx) = channel();

        // spawn a thread to read the messages from the engine
        thread::spawn(move || {
            // read everything it sent back
            loop {
                let message = {
                    let mut stdout = stdout_clone.lock().unwrap();

                    Self::recv_msg(&mut stdout)
                };

                // println!("MSG: {:?}", message);

                // convert the messages into Analysis
                let analysis = match message {
                    // convert this into a PossibleMove
                    UciMessage::Info(attrs) => {
                        let mut possible_move = PossibleMove::default();

                        for attr in attrs {
                            match attr {
                                UciInfoAttribute::Depth(d) => { possible_move.depth = d; },
                                UciInfoAttribute::Score { cp, .. } => { if let Some(score) = cp { possible_move.score = score; } },
                                UciInfoAttribute::Pv(moves) => { possible_move.moves = moves; }
                                // UciInfoAttribute::CurrMove(chess_move) => { info.push_str(&chess_move.to_string()); },
                                _ => ()
                            }
                        }

                        debug!("POSSIBLE MOVE: {:?}", possible_move);

                        Analysis::PossibleMove(possible_move)
                    },
                    UciMessage::BestMove { best_move, ponder } => {
                        println!("BEST MOVE: {}", best_move);

                        Analysis::BestMove(best_move)
                    }
                    _ => {
                        panic!("Unexpected message: {:?}", message)
                    }
                };

                let break_loop = if let Analysis::BestMove(_) = analysis { true } else { false };

                // send the analysis, check for disconnected receiver
                if let Err(send_err) = tx.send(analysis) {
                    debug!("SEND ERR: {:?}", send_err);

                    // tell the engine to stop
                    let mut stdin = stdin_clone.lock().unwrap();
                    Self::send_msg(&mut stdin, UciMessage::Stop);
                }

                // if we got the best move, then break out of the loop
                if break_loop {
                    break
                }
            }
        });

        // return the receiver side of the channel
        rx
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

        uci.analyze(&game, None);
    }
}