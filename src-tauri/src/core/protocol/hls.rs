use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use aes::Aes128;
use cbc::Decryptor;
use m3u8_rs::{MediaPlaylist, Playlist, VariantStream};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::PreferredQuality;
use crate::core::error::{CoreError, CoreResult};

/// 判断 URL 是否为 M3U8 流地址
pub fn is_m3u8(url: &str) -> bool {
    if let Ok(parsed) = Url::parse(url) {
        if parsed.path().ends_with(".m3u8") {
            return true;
        }
    }
    url.contains(".m3u8")
}

/// 辅助函数：将 m3u8_rs 解析结果转换为 CoreResult
pub fn parse_m3u8_playlist(bytes: &[u8]) -> CoreResult<Playlist> {
    m3u8_rs::parse_playlist_res(bytes).map_err(|e| CoreError::M3u8Parse(format!("{e:?}")))
}

/// 根据清晰度偏好从 Master Playlist 的所有变体流中挑选最优项
pub fn select_variant<'a>(
    variants: &'a [VariantStream],
    preferred_quality: PreferredQuality,
) -> Option<&'a VariantStream> {
    if variants.is_empty() {
        return None;
    }

    match preferred_quality {
        PreferredQuality::Highest => variants.iter().max_by_key(|v| {
            let h = v.resolution.map(|r| r.height).unwrap_or(0);
            (h, v.bandwidth)
        }),
        PreferredQuality::Lowest => variants.iter().min_by_key(|v| {
            let h = v.resolution.map(|r| r.height).unwrap_or(u64::MAX);
            (h, v.bandwidth)
        }),
        quality => {
            let target = match quality.target_height() {
                Some(t) => t,
                None => {
                    return variants.iter().max_by_key(|v| {
                        let h = v.resolution.map(|r| r.height).unwrap_or(0);
                        (h, v.bandwidth)
                    });
                }
            };

            let exact_match = variants
                .iter()
                .filter(|v| v.resolution.map_or(false, |r| r.height == target))
                .max_by_key(|v| v.bandwidth);

            if exact_match.is_some() {
                return exact_match;
            }

            let fallback_lower = variants
                .iter()
                .filter(|v| v.resolution.map_or(false, |r| r.height < target))
                .max_by_key(|v| (v.resolution.map(|r| r.height).unwrap_or(0), v.bandwidth));

            if fallback_lower.is_some() {
                return fallback_lower;
            }

            let fallback_lowest = variants
                .iter()
                .filter(|v| v.resolution.is_some())
                .min_by_key(|v| {
                    (
                        v.resolution.map(|r| r.height).unwrap_or(u64::MAX),
                        v.bandwidth,
                    )
                });

            if fallback_lowest.is_some() {
                return fallback_lowest;
            }

            variants.iter().max_by_key(|v| v.bandwidth)
        }
    }
}

type Aes128CbcDec = Decryptor<Aes128>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AesKeyInfo {
    pub method: String,
    pub uri: String,
    pub iv: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HlsSegment {
    pub index: usize,
    pub url: String,
    pub duration_secs: f64,
    pub key: Option<AesKeyInfo>,
}

/// 使用 AES-128-CBC 对 TS 分片数据进行解密
pub fn decrypt_aes128(ciphertext: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> CoreResult<Vec<u8>> {
    if ciphertext.is_empty() {
        return Ok(Vec::new());
    }
    let decryptor = Aes128CbcDec::new(key.into(), iv.into());
    let mut buf = ciphertext.to_vec();
    let plaintext = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| CoreError::Other(anyhow::anyhow!("AES-128 unpad failed: {:?}", e)))?;
    Ok(plaintext.to_vec())
}

/// 解析单层 MediaPlaylist 中的所有切片信息
pub fn extract_media_segments(base_url: &Url, media: MediaPlaylist) -> CoreResult<Vec<HlsSegment>> {
    let mut segments = Vec::with_capacity(media.segments.len());
    let mut current_key: Option<AesKeyInfo> = None;

    for (idx, seg) in media.segments.into_iter().enumerate() {
        if let Some(key) = seg.key {
            current_key = match key.method {
                m3u8_rs::KeyMethod::AES128 => {
                    let key_url = key
                        .uri
                        .and_then(|u| base_url.join(&u).ok())
                        .map(|u| u.to_string())
                        .unwrap_or_default();

                    let iv = key.iv.and_then(|iv_str| {
                        let clean = iv_str.trim_start_matches("0x").trim_start_matches("0X");
                        hex::decode(clean).ok()
                    });

                    Some(AesKeyInfo {
                        method: "AES-128".to_string(),
                        uri: key_url,
                        iv,
                    })
                }
                m3u8_rs::KeyMethod::None => None,
                _ => None,
            };
        }

        let full_url = base_url.join(&seg.uri)?.to_string();
        segments.push(HlsSegment {
            index: idx,
            url: full_url,
            duration_secs: seg.duration as f64,
            key: current_key.clone(),
        });
    }
    Ok(segments)
}

#[derive(Debug, Clone)]
pub enum ParsedPlaylist {
    Media(Vec<HlsSegment>),
    Master {
        sub_m3u8_url: Url,
        bandwidth: u64,
        resolution: Option<m3u8_rs::Resolution>,
    },
}

pub fn parse_playlist(
    base_url: &Url,
    bytes: &[u8],
    preferred_quality: PreferredQuality,
) -> CoreResult<ParsedPlaylist> {
    let playlist = parse_m3u8_playlist(bytes)?;
    match playlist {
        Playlist::MediaPlaylist(media) => {
            let segments = extract_media_segments(base_url, media)?;
            Ok(ParsedPlaylist::Media(segments))
        }
        Playlist::MasterPlaylist(master) => {
            let chosen_variant = select_variant(&master.variants, preferred_quality)
                .ok_or(CoreError::NoValidVariant)?;
            let sub_m3u8_url = base_url.join(&chosen_variant.uri)?;
            Ok(ParsedPlaylist::Master {
                sub_m3u8_url,
                bandwidth: chosen_variant.bandwidth,
                resolution: chosen_variant.resolution,
            })
        }
    }
}

pub fn rewrite_playlist_uris<F>(bytes: &[u8], mut transform_uri: F) -> CoreResult<String>
where
    F: FnMut(&str) -> String,
{
    let playlist = parse_m3u8_playlist(bytes)?;
    let mut out_buf = Vec::new();

    match playlist {
        Playlist::MasterPlaylist(mut master) => {
            for variant in &mut master.variants {
                variant.uri = transform_uri(&variant.uri);
            }
            master
                .write_to(&mut out_buf)
                .map_err(|e| CoreError::M3u8Parse(e.to_string()))?;
        }
        Playlist::MediaPlaylist(mut media) => {
            for seg in &mut media.segments {
                seg.uri = transform_uri(&seg.uri);
                if let Some(ref mut key) = seg.key {
                    if let Some(ref key_uri) = key.uri {
                        key.uri = Some(transform_uri(key_uri));
                    }
                }
                if let Some(ref mut map) = seg.map {
                    map.uri = transform_uri(&map.uri);
                }
            }
            media
                .write_to(&mut out_buf)
                .map_err(|e| CoreError::M3u8Parse(e.to_string()))?;
        }
    }

    String::from_utf8(out_buf)
        .map_err(|e| CoreError::M3u8Parse(format!("Invalid UTF-8 in m3u8: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use m3u8_rs::Resolution;

    fn make_variant(uri: &str, width: u64, height: u64, bandwidth: u64) -> VariantStream {
        VariantStream {
            uri: uri.to_string(),
            bandwidth,
            resolution: Some(Resolution { width, height }),
            ..Default::default()
        }
    }

    #[test]
    fn test_is_m3u8() {
        assert!(is_m3u8("https://example.com/live/playlist.m3u8"));
        assert!(is_m3u8("https://example.com/video.m3u8?token=123"));
        assert!(!is_m3u8("https://example.com/video.mp4"));
    }

    #[test]
    fn test_select_variant_highest_and_lowest() {
        let v1 = make_variant("720p.m3u8", 1280, 720, 2000000);
        let v2 = make_variant("1080p.m3u8", 1920, 1080, 5000000);
        let v3 = make_variant("480p.m3u8", 854, 480, 1000000);
        let variants = vec![v1, v2, v3];

        let highest = select_variant(&variants, PreferredQuality::Highest).unwrap();
        assert_eq!(highest.uri, "1080p.m3u8");

        let lowest = select_variant(&variants, PreferredQuality::Lowest).unwrap();
        assert_eq!(lowest.uri, "480p.m3u8");
    }

    #[test]
    fn test_select_variant_exact_and_fallback() {
        let v1 = make_variant("720p.m3u8", 1280, 720, 2000000);
        let v2 = make_variant("480p.m3u8", 854, 480, 1000000);
        let variants = vec![v1, v2];

        let selected = select_variant(&variants, PreferredQuality::P1080).unwrap();
        assert_eq!(selected.uri, "720p.m3u8");

        let selected_480 = select_variant(&variants, PreferredQuality::P480).unwrap();
        assert_eq!(selected_480.uri, "480p.m3u8");
    }

    #[test]
    fn test_select_variant_empty() {
        assert!(select_variant(&[], PreferredQuality::P1080).is_none());
    }

    #[test]
    fn test_parse_m3u8_playlist_error() {
        let invalid_content = b"This is clearly not an M3U8 file at all";
        let res = parse_m3u8_playlist(invalid_content);
        assert!(res.is_err());
        match res.unwrap_err() {
            CoreError::M3u8Parse(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Expected CoreError::M3u8Parse, got: {:?}", other),
        }
    }

    #[test]
    fn test_extract_media_segments_valid() {
        let base_url = Url::parse("https://example.com/live/").unwrap();
        let m3u8_text = "#EXTM3U\n#EXTINF:10.0,\nseg0.ts\n#EXTINF:10.0,\nseg1.ts\n#EXT-X-ENDLIST";
        let playlist = parse_m3u8_playlist(m3u8_text.as_bytes()).unwrap();
        if let Playlist::MediaPlaylist(media) = playlist {
            let segs = extract_media_segments(&base_url, media).unwrap();
            assert_eq!(segs.len(), 2);
            assert_eq!(segs[0].url, "https://example.com/live/seg0.ts");
            assert_eq!(segs[1].url, "https://example.com/live/seg1.ts");
        } else {
            panic!("Expected MediaPlaylist");
        }
    }

    #[test]
    fn test_decrypt_aes128() {
        use aes::cipher::block_padding::Pkcs7;
        use aes::cipher::{BlockEncryptMut, KeyIvInit};
        type Aes128CbcEnc = cbc::Encryptor<Aes128>;

        let key = [0x42u8; 16];
        let iv = [0x24u8; 16];
        let plaintext = b"Hello, MPEG-TS AES-128 Decryption!";

        let encryptor = Aes128CbcEnc::new(&key.into(), &iv.into());
        let mut buf = [0u8; 64];
        buf[..plaintext.len()].copy_from_slice(plaintext);
        let ct = encryptor
            .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
            .unwrap();

        let pt = decrypt_aes128(ct, &key, &iv).unwrap();
        assert_eq!(pt, plaintext);
    }
}
