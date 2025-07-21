
use crate::core::{Editor, EditorConfig};

use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType};

use nano_crl2::core::lexer::{LexicalElement, Token};

pub const SEMANTIC_TOKEN_MAP: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION, // 0
    SemanticTokenType::VARIABLE, // 1
    SemanticTokenType::COMMENT, // 2
    SemanticTokenType::NUMBER, // 3
    SemanticTokenType::KEYWORD, // 4
    SemanticTokenType::OPERATOR, // 5
    SemanticTokenType::PARAMETER, // 6
    SemanticTokenType::TYPE, // 7
];

/// Converts a list of plain tokens to semantic tokens, through a naive
/// per-token pass.
/// 
/// This function does not attempt to utilise any sort of semantic information.
pub fn get_semantic_tokens_from_tokens(
    tokens: &Vec<Token>,
    editor_config: &EditorConfig,
) -> Vec<SemanticToken> {
    let mut result = Vec::new();
    let mut curr_line = 0;
    let mut curr_char = 0;
    let mut delta_line = 0;
    let mut delta_start = 0;
    for token in tokens {
        delta_line += token.loc.get_start_line() - curr_line;
        if delta_line == 0 {
            delta_start += token.loc.get_start_char() - curr_char
        } else {
            delta_start = token.loc.get_start_char()
        }
        if let Some(i) = get_semantic_token_index_from_lexical_element(&token.value) {
            if should_add_semantic_token(editor_config, i) {
                result.push(SemanticToken {
                    delta_start,
                    delta_line,
                    length: token.value.get_length() as u32,
                    token_type: i as u32,
                    token_modifiers_bitset: 0,
                });
                delta_line = 0;
                delta_start = 0;
            }
        }
        curr_line = token.loc.get_start_line();
        curr_char = token.loc.get_start_char();
    }
    result
}

fn should_add_semantic_token(
    editor_config: &EditorConfig,
    semantic_token_index: usize,
) -> bool {
    if editor_config.editor == Editor::VsCode {
        // in vscode, keywords are handled by the extension on the client-side
        semantic_token_index != 4 && semantic_token_index != 2
    } else {
        true
    }
}

fn get_semantic_token_index_from_lexical_element(
    value: &LexicalElement,
) -> Option<usize> {
    use LexicalElement::*;

    match value {
        OpeningParen | ClosingParen | OpeningBracket | ClosingBracket |
        OpeningBrace | ClosingBrace | Tilde | ExclamationMark | AtSign |
        HashSign | DollarSign | Circonflex | Ampersand | Asterisk | Dash |
        Equals | Plus | Pipe | Semicolon | Colon | Comma | LessThan | Period |
        GreaterThan | Slash | QuestionMark | DoublePipe | DoubleAmpersand |
        DoubleEquals | NotEquals | LessThanEquals | GreaterThanEquals |
        Diamond | Arrow | ThickArrow | ConsOperator | SnocOperator | Concat |
        DoublePipeUnderscore => Some(5),

        Act | Allow | Block | Comm | Cons | Delay | Div | End | Eqn | Exists |
        Forall | Glob | Hide | If | In | Init | Lambda | Map | Mod | Mu | Nu |
        Pbes | Proc | Rename | Sort | Struct | Sum | Val | Var | Whr | Yaled |
        Delta | False | Nil | Tau | True => Some(4),

        Bag | Bool | FBag | FSet | Int | List | Nat | Pos | Real | Set => Some(7),

        Identifier(_) => Some(1), // assumption, though it could be anything
        Comment(_) => Some(2),
        DocComment(_) => Some(2),
        Integer(_) => Some(3),
    }
}
