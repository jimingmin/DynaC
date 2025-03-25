use std::{collections::HashMap, iter::Peekable, str::Chars, sync::{mpsc::TryRecvError, OnceLock}, thread::current};
use strum_macros::{EnumString, Display};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
pub enum TokenType {
    // Single-character tokens.
    TokenLeftParen,
    TokenRightParen,
    TokenLeftBrace,
    TokenRightBrace,
    TokenComma,
    TokenDot,
    TokenMinus,
    TokenPlus,
    TokenSemicolon,
    TokenSlash,
    TokenStar,

    // One or two character tokens.
    TokenBang,
    TokenBangEqual,
    TokenEqual,
    TokenEqualEqual,
    TokenGreater,
    TokenGreaterEqual,
    TokenLess,
    TokenLessEqual,

    // Literals.
    TokenIdentifier,
    TokenString,
    TokenNumber,

    // Keywords.
    TokenAnd,
    TokenClass,
    TokenElse,
    TokenFalse,
    TokenFor,
    TokenFun,
    TokenIf,
    TokenNil,
    TokenOr,
    TokenPrint,
    TokenReturn,
    TokenSuper,
    TokenThis,
    TokenTrue,
    TokenVar,
    TokenWhile,

    TokenError,
    TokenEof,
}

static KEYWORDS: phf::Map<&'static str, TokenType> = phf::phf_map! {
    "and" => TokenType::TokenAnd,
    "class" => TokenType::TokenClass,
    "else" => TokenType::TokenElse,
    "if" => TokenType::TokenIf,
    "nil" => TokenType::TokenNil,
    "or" => TokenType::TokenOr,
    "print" => TokenType::TokenPrint,
    "return" => TokenType::TokenReturn,
    "super" => TokenType::TokenSuper,
    "var" => TokenType::TokenVar,
    "while" => TokenType::TokenWhile,
    "for" => TokenType::TokenFor,
    "false" => TokenType::TokenFalse,
    "fun" => TokenType::TokenFun,
    "this" => TokenType::TokenThis,
    "true" => TokenType::TokenTrue,
};

#[derive(Debug)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    token_type: Option<TokenType>,
    is_end: bool,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            token_type: None,
            is_end: false,
        }
    }
}

static TRIE_ROOT: OnceLock<TrieNode> = OnceLock::new();

#[derive(Debug)]
pub struct Token<'a> {
    pub token_type: TokenType,
    pub value: &'a str,
    pub line: usize,
}

pub struct Scanner<'a> {
    source: &'a str,
    chars: Peekable<Chars<'a>>,
    start: usize,
    current: usize,
    line: usize
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Box<Scanner<'a>> {
        TRIE_ROOT.get_or_init(|| {
            let mut root = TrieNode::new();
            for (keyword, token) in KEYWORDS.entries() {
                let mut current_node = &mut root;
                for c in keyword.chars() {
                    current_node = current_node.children.entry(c).or_insert(TrieNode::new());
                }
                current_node.token_type = Some(*token);
                current_node.is_end = true;
            }
            root
        });

        let chars = source.chars().peekable();
        Box::new(Scanner {
            source,
            chars,
            start: 0,
            current: 0,
            line: 1,
        })
    }

    pub fn scan_token(&mut self) -> Token<'a> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_end() {
            return self.make_token(TokenType::TokenEof);
        }

        let c = self.advance();

        if Self::is_alpha(c) {
            return self.make_identifier_token();
        }

        if Self::is_digit(c) {
            return self.make_number_token();
        }

        match c {
            '(' => self.make_token(TokenType::TokenLeftParen),
            ')' => self.make_token(TokenType::TokenRightParen),
            '{' => self.make_token(TokenType::TokenLeftBrace),
            '}' => self.make_token(TokenType::TokenRightBrace),
            ';' => self.make_token(TokenType::TokenSemicolon),
            ',' => self.make_token(TokenType::TokenComma),
            '.' => self.make_token(TokenType::TokenDot),
            '-' => self.make_token(TokenType::TokenMinus),
            '+' => self.make_token(TokenType::TokenPlus),
            '/' => self.make_token(TokenType::TokenSlash),
            '*' => self.make_token(TokenType::TokenStar),
            '"' => self.make_string_token(),
            '!' => {
                if self.match_char('=') {
                    self.make_token(TokenType::TokenBangEqual)
                } else {
                    self.make_token(TokenType::TokenBang)
                }
            },
            '=' => {
                if self.match_char('=') {
                    self.make_token(TokenType::TokenEqualEqual)
                } else {
                    self.make_token(TokenType::TokenEqual)
                }
            },
            '<' => {
                if self.match_char('=') {
                    self.make_token(TokenType::TokenLessEqual)
                } else {
                    self.make_token(TokenType::TokenLess)
                }
            },
            '>' => {
                if self.match_char('=') {
                    self.make_token(TokenType::TokenGreaterEqual)
                } else {
                    self.make_token(TokenType::TokenGreater)
                }
            },
            _ => self.error_token("Unexpected character."),
        }
    }

    fn is_digit(ch: char) -> bool {
        ch.is_ascii_digit()
    }

    fn is_alpha(ch: char) -> bool {
        ch.is_ascii_alphabetic() || ch == '_'
    }

    fn identifier_type(&mut self) -> TokenType {
        //TokenType::TokenIdentifier
        let keyword = self.check_keyword();
        match keyword {
            Some(TokenType::TokenError) => TokenType::TokenError,
            Some(token_type) => token_type,
            None => TokenType::TokenError,
        }
    }

    fn make_identifier_token(&mut self) -> Token<'a> {
        loop {
            match self.peek() {
                Some(c) if Self::is_alpha(*c) || Self::is_digit(*c) => self.advance(),
                _ => break,
            };
        }
        let token_type = self.identifier_type();
        self.make_token(token_type)
    }

    fn make_number_token(&mut self) -> Token<'a> {
        // 读取整数部分
        loop {
            match self.peek() {
                Some(c) if Self::is_digit(*c) => self.advance(),
                _ => break,
            };
        }

        // 处理小数点
        let current = self.peek();
        if let Some('.') = current {
            if let Some(c) = self.peek_next() {
                if Self::is_digit(c) {
                    self.advance(); // 跳过小数点
                    // 读取小数部分
                    while let Some(ch) = self.peek() {
                        if Self::is_digit(*ch) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        self.make_token(TokenType::TokenNumber)
    }

    fn make_string_token(&mut self) -> Token<'a> {
        while let Some(ch) = self.peek() {
            if *ch == '\n' {
                self.line += 1;
            } else if *ch == '"' {
                break;
            } else if self.is_end() {
                return self.error_token("Unterminated string.");
            }

            self.advance();
        }

        self.advance();
        return self.make_token(TokenType::TokenString);
    }

    fn skip_whitespace(&mut self) {
        loop {
            // 在同一个可变借用作用域内缓存结果
            // let (current_char, next_char) = {
            //     let current = self.peek();
            //     let next = self.peek_next();
            //     (current, next)
            // };
            let next_char = self.peek_next();

            match (self.peek(), next_char) {
                (Some(c), _) if c.is_whitespace() => {
                    self.advance();
                }
                (Some('/'), Some('/')) => {
                    while let Some(ch) = self.peek() {
                        if *ch == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                (Some(_), _) | (None, _) => return,
            }
        }
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn peek_next(&self) -> Option<char> {
        if self.is_end() {
            return None;
        }

        let mut iter = self.chars.clone();
        iter.next();
        iter.next()
    }

    fn is_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn make_token(&self, token_type: TokenType) -> Token<'a> {
        Token {
            token_type, 
            value: &self.source[self.start..self.current], 
            line: self.line
        }
    }

    fn error_token(&self, reason: &'static str) -> Token<'a> {
        Token {
            token_type: TokenType::TokenError,
            value: reason,
            line: self.line
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_end() {
            return false;
        }

        if let Some(&next_char) = self.chars.peek() {
            if next_char == expected {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check_keyword(&mut self) -> Option<TokenType> {
        let trie_root = TRIE_ROOT.get().expect("Trie not initialized");
        let mut current_node = trie_root;

        let mut keyword = Some(TokenType::TokenIdentifier);
        let substring = &self.source[self.start..self.current];
        for ch in substring.chars() {
            match current_node.children.get(&ch) {
                Some(child) => {
                    current_node = child;
                    if current_node.is_end {
                        keyword = current_node.token_type;
                    }
                },
                None => return Some(TokenType::TokenIdentifier),
            }
        }
        if current_node.is_end {
            keyword = current_node.token_type;
        }
        keyword
    }

    fn advance(&mut self) -> char {
        if let Some(next_char) = self.chars.next() {
            self.current += next_char.len_utf8();
            next_char
        } else {
            '\0'
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::scanner::TokenType;

    use super::Scanner;

    #[test]
    fn test_check_keyword() {
        let mut scanner = Scanner::new("this is for if fun  fun1 forfor %%dadf");
        let mut token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenThis);
        assert!(token.value == "this");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenIdentifier);
        assert!(token.value == "is");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenFor);
        assert!(token.value == "for");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenIf);
        assert!(token.value == "if");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenFun);
        assert!(token.value == "fun");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenIdentifier);
        assert!(token.value == "fun1");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenIdentifier);
        assert!(token.value == "forfor");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::TokenError);
    }
}
