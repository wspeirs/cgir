use chess::{ChessMove, Board, Color, MoveGen, BitBoard, Square, Piece, BoardStatus};
use itertools::Itertools;
use std::collections::HashSet;

pub fn to_notation(chess_move :&ChessMove, board :&Board) -> String {
    println!("{} -> {}", chess_move.get_source(), chess_move.get_dest());

    let piece = board.piece_on(chess_move.get_source());

    // this is probably an error, so just return the source->destination
    if piece.is_none() {
        return format!("{}", chess_move);
    }

    // check for a stalemate, that's a draw
    if board.status() == BoardStatus::Stalemate {
        return "(=)".to_string();
    }

    let piece = piece.unwrap();
    let dest = chess_move.get_dest().to_string();

    // start the return value with the piece
    let mut ret = if piece == Piece::Pawn {
        "".to_string()
    } else {
        piece.to_string(Color::White)
    };

    // generate all the valid moves
    let mut move_gen = MoveGen::new_legal(board);

    // only want moves that land on the same square
    move_gen.set_iterator_mask(BitBoard::from_square(chess_move.get_dest()));

    // filter out moves that aren't with the same piece
    let mut file_rank_same_pieces = move_gen
        .filter(|mv| board.piece_on(mv.get_source()).unwrap() == piece)
        .map(|mv| (mv.get_source().get_file(), mv.get_source().get_rank()))
        .collect::<Vec<_>>();

    // have to dedup because Rank & File don't impl Eq :-(
    file_rank_same_pieces.sort_unstable_by(|(f1, r1), (f2, r2)| f1.to_index().cmp(&f2.to_index()).then(r1.to_index().cmp(&r2.to_index())));
    file_rank_same_pieces.dedup();

    println!("SAME: {:?}", file_rank_same_pieces);

    // check to see if their are 2 pieces on the same files
    if file_rank_same_pieces.len() > 1 {
        if file_rank_same_pieces.iter().map(|(f, r)| f.to_index()).counts().values().any(|v| *v > 1) {
            // check to see if there are 2 pieces on the same rank
            ret += if file_rank_same_pieces.iter().map(|(f, r)| r.to_index()).counts().values().any(|v| *v > 1) {
                // we need a full coordinate here
                format!("{}", chess_move.get_source())
            } else {
                // rank is enough to disambiguate
                format!("{}", chess_move.get_source().get_rank().to_index() + 1)
            }.as_str()
        } else {
            // the file is enough to disambiguate
            ret += format!("{}", format!("{:?}", chess_move.get_source().get_file()).to_ascii_lowercase()).as_str()
        }
    }

    // add an 'x' if we have a capture
    if board.piece_on(chess_move.get_dest()).is_some() {
        ret += "x";
    }

    // add on the destination
    ret += dest.to_string().as_str();

    // check to see if we have a promotion
    if let Some(p) = chess_move.get_promotion() {
        ret += format!("={}", p.to_string(Color::White)).as_str();
    }

    // check for mate
    if board.status() == BoardStatus::Checkmate {
        ret += "#";
    } else if board.checkers().popcnt() != 0 {
        ret += "+"; // see if there's a check
    }

    ret
}


#[cfg(test)]
mod tests {
    use chess::{BoardBuilder, Board, Piece, Color, Rank, Square, ChessMove};
    use std::convert::TryFrom;
    use crate::chess_utils::to_notation;

    fn make_board() -> Board {
        Board::try_from(BoardBuilder::new()
            .piece(Square::E1, Piece::Knight, Color::Black)
            .piece(Square::H1, Piece::Queen, Color::White)
            .piece(Square::A7, Piece::Pawn, Color::White)
            .piece(Square::H7, Piece::Pawn, Color::Black)
            .piece(Square::E4, Piece::Queen, Color::White)
            .piece(Square::H4, Piece::Queen, Color::White)
            .piece(Square::C6, Piece::King, Color::White)
            .piece(Square::G7, Piece::King, Color::Black)
            .piece(Square::D8, Piece::Rook, Color::Black)
            .piece(Square::H8, Piece::Rook, Color::Black)
            .side_to_move(Color::White)
        ).unwrap()
    }

    #[test]
    fn standard_move() {
        let board = make_board();

        assert_eq!("Qh1xe1".to_string(), to_notation(&ChessMove::new(Square::H1, Square::E1, None), &board));
        assert_eq!("Q1h2".to_string(), to_notation(&ChessMove::new(Square::H1, Square::H2, None), &board));
        assert_eq!("Qef4".to_string(), to_notation(&ChessMove::new(Square::E4, Square::F4, None), &board));
        assert_eq!("Qd3".to_string(), to_notation(&ChessMove::new(Square::E4, Square::D3, None), &board));
        assert_eq!("a8=Q".to_string(), to_notation(&ChessMove::new(Square::A7, Square::A8, Some(Piece::Queen)), &board));
        // assert_eq!("Qh6+".to_string(), to_notation(&ChessMove::new(Square::H4, Square::H6, None), &board));
    }
}