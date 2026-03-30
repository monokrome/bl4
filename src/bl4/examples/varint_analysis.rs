use bl4::serial::Token;
use bl4::ItemSerial;
use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let serial = line.trim();
        if serial.is_empty() || !serial.starts_with("@Ug") {
            continue;
        }

        let Ok(item) = ItemSerial::decode(serial) else {
            println!("{}\tDECODE_FAILED", serial);
            continue;
        };

        // Full token stream
        let token_strs: Vec<String> = item
            .tokens
            .iter()
            .map(|t| match t {
                Token::Var { val: v, encoding: bl4::serial::VarEncoding::Int } => format!("VI({})", v),
                Token::Var { val: v, encoding: bl4::serial::VarEncoding::Bit } => format!("VB({})", v),
                Token::Separator => "SEP".to_string(),
                Token::SoftSeparator => "SOFT".to_string(),
                Token::Part { index, values, .. } => {
                    if values.is_empty() {
                        format!("P({})", index)
                    } else {
                        let vs: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                        format!("P({}:{})", index, vs.join(","))
                    }
                }
                Token::String(s) => format!("S(len={})", s.len()),
            })
            .collect();

        // Extract all VarInts and VarBits in order
        let header_vals: Vec<String> = item
            .tokens
            .iter()
            .take_while(|t| !matches!(t, Token::Separator))
            .map(|t| match t {
                Token::Var { val: v, encoding: bl4::serial::VarEncoding::Int } => format!("vi={}", v),
                Token::Var { val: v, encoding: bl4::serial::VarEncoding::Bit } => format!("vb={}", v),
                Token::SoftSeparator => "soft".to_string(),
                _ => "?".to_string(),
            })
            .collect();

        println!(
            "{}\theader=[{}]\ttokens={}",
            serial,
            header_vals.join(", "),
            token_strs.join(" ")
        );
    }
}
