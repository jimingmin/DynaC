use std::{collections::HashMap, iter::Peekable, str::Chars, sync::{mpsc::TryRecvError, OnceLock}, thread::current};
use strum_macros::{EnumString, Display};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
pub enum TokenType {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    Error,
    Eof,
}

static KEYWORDS: phf::Map<&'static str, TokenType> = phf::phf_map! {
    "and" => TokenType::And,
    "class" => TokenType::Class,
    "else" => TokenType::Else,
    "if" => TokenType::If,
    "nil" => TokenType::Nil,
    "or" => TokenType::Or,
    "print" => TokenType::Print,
    "return" => TokenType::Return,
    "super" => TokenType::Super,
    "var" => TokenType::Var,
    "while" => TokenType::While,
    "for" => TokenType::For,
    "false" => TokenType::False,
    "fun" => TokenType::Fun,
    "this" => TokenType::This,
    "true" => TokenType::True,
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

#[derive(Debug, Clone)]
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
            return self.make_token(TokenType::Eof);
        }

        let c = self.advance();

        if Self::is_alpha(c) {
            return self.make_identifier_token();
        }

        if Self::is_digit(c) {
            return self.make_number_token();
        }

        match c {
            '(' => self.make_token(TokenType::LeftParen),
            ')' => self.make_token(TokenType::RightParen),
            '{' => self.make_token(TokenType::LeftBrace),
            '}' => self.make_token(TokenType::RightBrace),
            ';' => self.make_token(TokenType::Semicolon),
            ',' => self.make_token(TokenType::Comma),
            '.' => self.make_token(TokenType::Dot),
            '-' => self.make_token(TokenType::Minus),
            '+' => self.make_token(TokenType::Plus),
            '/' => self.make_token(TokenType::Slash),
            '*' => self.make_token(TokenType::Star),
            '"' => self.make_string_token(),
            '!' => {
                if self.match_char('=') {
                    self.make_token(TokenType::BangEqual)
                } else {
                    self.make_token(TokenType::Bang)
                }
            },
            '=' => {
                if self.match_char('=') {
                    self.make_token(TokenType::EqualEqual)
                } else {
                    self.make_token(TokenType::Equal)
                }
            },
            '<' => {
                if self.match_char('=') {
                    self.make_token(TokenType::LessEqual)
                } else {
                    self.make_token(TokenType::Less)
                }
            },
            '>' => {
                if self.match_char('=') {
                    self.make_token(TokenType::GreaterEqual)
                } else {
                    self.make_token(TokenType::Greater)
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
            Some(TokenType::Error) => TokenType::Error,
            Some(token_type) => token_type,
            None => TokenType::Error,
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

        self.make_token(TokenType::Number)
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
        return self.make_token(TokenType::String);
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
            token_type: TokenType::Error,
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

        let mut keyword = Some(TokenType::Identifier);
        let substring = &self.source[self.start..self.current];
        for ch in substring.chars() {
            match current_node.children.get(&ch) {
                Some(child) => {
                    current_node = child;
                    if current_node.is_end {
                        keyword = current_node.token_type;
                    }
                },
                None => return Some(TokenType::Identifier),
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
        assert!(token.token_type == TokenType::This);
        assert!(token.value == "this");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::Identifier);
        assert!(token.value == "is");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::For);
        assert!(token.value == "for");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::If);
        assert!(token.value == "if");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::Fun);
        assert!(token.value == "fun");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::Identifier);
        assert!(token.value == "fun1");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::Identifier);
        assert!(token.value == "forfor");

        token = scanner.scan_token();
        assert!(token.token_type == TokenType::Error);
    }

    #[test]
    fn test_scan_token() {
        let source = 
        "var a = 1;
        var b = \"this is a string\";
        while(true) {
            if (a == 1) {
                print(a);
            }

            var c = a and 1 or 2;
            for (var d = 1; d <= 5; ++d) {
                a = a + 1;
            }
        }
        fun test() {
            var a = 1 + 2 * 3 / 4 - -5;
            if a > 1 {
                a = -a;
            } else {
                a = a - 1;
            }
            return;
        }
        ";
        let mut scanner = Scanner::new(source);
        while let token =  scanner.scan_token() {
            println!("token is : {:?}", token);
            if token.token_type == TokenType::Error {
                assert!(false);
            }
            if token.token_type == TokenType::Eof {
                break;
            }
        };
    }
}
