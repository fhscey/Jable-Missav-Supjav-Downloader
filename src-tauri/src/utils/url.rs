use url::Url;

pub fn url_encode(key_word: &str) -> String {
    url::form_urlencoded::byte_serialize(key_word.as_bytes()).collect()
}

/// 通用 URL Query 参数拼接与覆盖工具函数
pub fn merge_query_pairs(raw_url: &str, new_pairs: &[(&str, String)]) -> Option<String> {
    let mut parsed = Url::parse(raw_url).ok()?;

    if new_pairs.is_empty() {
        return Some(raw_url.to_string());
    }

    let mut merged: Vec<(String, String)> = Vec::new();
    let mut replaced_keys = std::collections::HashSet::new();

    for (k, v) in parsed.query_pairs() {
        if let Some((_, new_v)) = new_pairs.iter().find(|(nk, _)| *nk == k.as_ref()) {
            if !replaced_keys.contains(k.as_ref()) {
                merged.push((k.to_string(), new_v.clone()));
                replaced_keys.insert(k.to_string());
            }
        } else {
            merged.push((k.to_string(), v.to_string()));
        }
    }

    for (k, v) in new_pairs {
        if !replaced_keys.contains(*k) {
            merged.push((k.to_string(), v.clone()));
        }
    }

    parsed.set_query(None);
    {
        let mut q_pairs = parsed.query_pairs_mut();
        for (k, v) in merged {
            q_pairs.append_pair(&k, &v);
        }
    }

    Some(parsed.to_string())
}

/// 域名替换工具函数：将 raw_url 的协议、主机与端口替换为 target_domain 的对应项
pub fn replace_url_domain(raw_url: &str, target_domain: &str) -> Option<String> {
    let mut parsed = Url::parse(raw_url).ok()?;
    let target_parsed = Url::parse(target_domain).ok()?;
    let _ = parsed.set_scheme(target_parsed.scheme());
    let _ = parsed.set_host(target_parsed.host_str());
    let _ = parsed.set_port(target_parsed.port());
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_url_domain() {
        let raw = "https://jable.tv/videos/ssis-001/?lang=zh";
        let target = "https://fs1.app";
        let replaced = replace_url_domain(raw, target).unwrap();
        assert_eq!(replaced, "https://fs1.app/videos/ssis-001/?lang=zh");

        let raw_missav = "https://missav.ws/dm132/ssis-001";
        let target_missav = "https://missav.ai";
        let replaced_missav = replace_url_domain(raw_missav, target_missav).unwrap();
        assert_eq!(replaced_missav, "https://missav.ai/dm132/ssis-001");
    }

    #[test]
    fn test_replace_url_domain_with_port() {
        let raw = "http://localhost:3000/test/path";
        let target = "https://example.com";
        let replaced = replace_url_domain(raw, target).unwrap();
        assert_eq!(replaced, "https://example.com/test/path");
    }

    #[test]
    fn test_merge_query_pairs() {
        let url = "https://jable.tv/hot/?from_page=1";
        let pairs = vec![
            ("from_page", "2".to_string()),
            ("sort_by", "video_viewed".to_string()),
        ];
        let built = merge_query_pairs(url, &pairs).unwrap();
        assert_eq!(
            built,
            "https://jable.tv/hot/?from_page=2&sort_by=video_viewed"
        );

        let url_no_query = "https://missav.ws/new";
        let pairs2 = vec![("page", "3".to_string()), ("sort", "views".to_string())];
        let built2 = merge_query_pairs(url_no_query, &pairs2).unwrap();
        assert_eq!(built2, "https://missav.ws/new?page=3&sort=views");

        let invalid_url = "not a valid url";
        assert_eq!(merge_query_pairs(invalid_url, &pairs), None);
    }
}
