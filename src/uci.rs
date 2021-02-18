use std::process::{Command, Stdio, ChildStdin, ChildStdout};
use std::io::{BufReader, Write, BufRead};
use std::thread;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Mutex, Arc};

use log::{debug, warn};
use vampirc_uci::{ByteVecUciMessage, UciMessage, parse_one, UciFen, UciSearchControl, UciTimeControl, UciInfoAttribute};
use chess::{Game, ChessMove};
use std::collections::HashMap;
use itertools::Itertools;

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
    multi_pv: u16,
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

        // bump the number of threads so it works faster :-)
        Self::send_msg(&mut stdin, UciMessage::SetOption {name: "Threads".to_string(), value: Some("4".to_string())});

        // also tell it to use analysis mode
        Self::send_msg(&mut stdin, UciMessage::SetOption { name: "UCI_AnalyseMode".to_string(), value: Some("true".to_string()) });

        // tell it to do multiple lines?
        Self::send_msg(&mut stdin, UciMessage::SetOption { name: "MultiPV".to_string(), value: Some("5".to_string() )});

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

    /// Given a game, and additional moves to consider, and a depth; analyze the game
    /// A Receiver of Analysis structs is returned
    /// When the depth is reached (None for infinite), or the Receiver is dropped,
    /// the engine will stop its analysis
    pub fn analyze(&mut self, game :&Game, moves: Vec<ChessMove>, depth :Option<u8>) -> Receiver<Analysis> {
        debug!("CUR POS: {}", game.current_position());

        { // scope our lock
            let mut stdin = self.stdin.lock().unwrap();

            // set the position
            Self::send_msg(&mut stdin, UciMessage::Position {
                startpos: false,
                fen: Some(UciFen(game.current_position().to_string())),
                moves
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

                // debug!("MSG: {:?}", message);

                // convert the messages into Analysis
                let analysis = match message {
                    // convert this into a PossibleMove
                    UciMessage::Info(attrs) => {
                        let mut possible_move = PossibleMove::default();

                        // set this to 1 just in case we didn't set the MultiPV option above
                        possible_move.multi_pv = 1;

                        // debug!("ATTRS: {:?}", attrs);

                        for attr in attrs {
                            match attr {
                                UciInfoAttribute::Depth(d) => { possible_move.depth = d; },
                                UciInfoAttribute::Score { cp, mate, .. } => { if let Some(score) = cp { possible_move.score = score; } },
                                UciInfoAttribute::Pv(moves) => { possible_move.moves = moves; }
                                UciInfoAttribute::MultiPv(multi_pv) => { possible_move.multi_pv = multi_pv; }
                                // UciInfoAttribute::CurrMove(chess_move) => { info.push_str(&chess_move.to_string()); },
                                UciInfoAttribute::String(s) => { eprintln!("STR: {}", s); }
                                _ => ()
                            }
                        }

                        // debug!("POSSIBLE MOVE: {} {} {:?}",
                        //        possible_move.depth,
                        //        possible_move.score,
                        //        possible_move.moves.iter().map(|mv| format!("{}", mv)).collect::<Vec<_>>());

                        Analysis::PossibleMove(possible_move)
                    },
                    UciMessage::BestMove { best_move, ponder } => {
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

    /// Given a game, proposed move, and a depth, check to see if there's a blunder
    /// The function returns (bool, Vec<(Score, Move)>)
    /// The boolean indicates if there's a blunder or not
    /// The Vec has the list of moves in sorted order by score
    pub fn check_for_blunder(&mut self, game :&Game, proposed_move: ChessMove, depth: u8) -> (bool, Vec<(i32, ChessMove)>) {
        // go through first and get all of the proposed "best" moves
        let rx = self.analyze(game, vec![], Some(depth));
        let mut best_moves = HashMap::new();

        for analysis in rx {
            if let Analysis::PossibleMove(pm) = analysis {
                best_moves.insert(pm.multi_pv, pm);
            }
        }

        // convert from the HashMap to a Vec
        let best_moves = best_moves
            .into_iter()
            .map(|(_mpv, pm)| (pm.score, pm.moves[0]))
            .sorted_by_key(|(score, mv)| *score)
            .collect_vec();

        debug!("BEST MOVES");
        best_moves.iter().for_each(|(score, mv)| debug!("{}: {}", score, mv));

        // check to see if this move is one of the "best" moves
        if best_moves.iter().any(|(score, mv)| *mv == proposed_move) {
            return (false, best_moves)
        }

        // add the move, and perform the analysis
        let rx = self.analyze(game, vec![proposed_move], Some(depth));
        let mut best_responses = HashMap::new();

        for analysis in rx {
            if let Analysis::PossibleMove(pm) = analysis {
                best_responses.insert(pm.multi_pv, pm);
            }
        }

        debug!("BEST RESPONSES");
        best_responses.iter().for_each(|(_mpv, mv)| debug!("{}: {}", mv.score, mv.moves[0]));

        // get the score of the best response
        let best_response_score = best_responses
            .into_iter()
            .map(|(_mpv, pm)| pm.score)
            .sorted()
            .next()
            .expect("Did not find any responses");

        debug!("BEST RESPONSE SCORE: {}", best_response_score);

        return (false, vec![])
    }
}


#[cfg(test)]
mod uci_tests {
    use std::process::Command;
    use std::convert::TryFrom;
    use std::str::FromStr;

    use chess::{Game, ChessMove, Square};
    use crate::uci::{Uci, Analysis};
    use simple_logger::SimpleLogger;
    use std::time::Duration;

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
        SimpleLogger::new().init().unwrap();
        let mut cmd = Command::new("/usr/games/ethereal-chess");
        let mut uci = Uci::start_engine(&mut cmd);
        let game = Game::from_str("r1bqkb1r/pppp1ppp/2n2n2/4p3/4P3/3P1P2/PPP3PP/RNBQKBNR w KQkq - 0 1").expect("Error creating game");

        let rx = uci.analyze(&game, vec![], Some(7));

        for analysis in rx {
            if let Analysis::BestMove(mv) = analysis {
                println!("{:?}", analysis);
            }
        }

        let rx = uci.analyze(&game, vec![], Some(7));

        for analysis in rx {
            if let Analysis::BestMove(mv) = analysis {
                println!("{:?}", analysis);
            }
        }
    }

    #[test]
    fn check_for_blunder_test() {
        SimpleLogger::new().init().unwrap();
        let mut cmd = Command::new("/usr/games/stockfish");
        let mut uci = Uci::start_engine(&mut cmd);
        let game = Game::from_str("r1bqkb1r/pppp1ppp/2n2n2/4p3/4P3/3P1P2/PPP3PP/RNBQKBNR w KQkq - 0 1").expect("Error creating game");
        let blunder_move = ChessMove::new(Square::B1, Square::B3, None);

        let (mv, score) = uci.check_for_blunder(&game, blunder_move, 18);
        println!("{}", mv);
    }

}