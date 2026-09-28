use std::collections::HashMap;

pub fn unpack_js_eval(script_text: &str) -> Option<String> {
    let re = regex::Regex::new(
        r"eval\s*\(\s*function\s*\(\s*p\s*,\s*a\s*,\s*c\s*,\s*k\s*,\s*e\s*,\s*d\s*\)\s*\{[\s\S]*?\}\s*\(\s*'([\s\S]*?)'\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*'([\s\S]*?)'\s*\.split\s*\(\s*'\|'\s*\)"
    ).ok()?;

    let caps = re.captures(script_text)?;
    let packed = caps.get(1)?.as_str();
    let a: u32 = caps.get(2)?.as_str().parse().ok()?;
    let c: u32 = caps.get(3)?.as_str().parse().ok()?;
    let keys: Vec<&str> = caps.get(4)?.as_str().split('|').collect();

    if a <= 1 || c > 200000 {
        return None;
    }

    fn to_base(mut n: u32, base: u32) -> String {
        if n == 0 {
            return "0".to_string();
        }
        let digits = "0123456789abcdefghijklmnopqrstuvwxyz";
        let mut s = String::new();
        while n > 0 {
            let rem = (n % base) as usize;
            s.insert(0, digits.chars().nth(rem).unwrap());
            n /= base;
        }
        s
    }

    let mut lookup = HashMap::new();
    for i in 0..c {
        let key = to_base(i, a);
        let val = if i < keys.len() as u32 && !keys[i as usize].is_empty() {
            keys[i as usize].to_string()
        } else {
            key.clone()
        };
        lookup.insert(key, val);
    }

    let re_word = regex::Regex::new(r"\b\w+\b").ok()?;
    let unpacked = re_word.replace_all(packed, |caps: &regex::Captures| {
        let word = &caps[0];
        lookup
            .get(word)
            .cloned()
            .unwrap_or_else(|| word.to_string())
    });

    Some(unpacked.to_string())
}
