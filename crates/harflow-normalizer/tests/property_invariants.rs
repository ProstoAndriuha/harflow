use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use harflow_har::{stream_har_reader, ImportLimits};
use harflow_normalizer::normalize_har_reader;

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_range(&mut self, max_exclusive: usize) -> usize {
        usize::try_from(self.next_u64() % (max_exclusive as u64)).unwrap_or(0)
    }
}

#[test]
fn fuzz_seed_streaming_parser_never_panics_on_mutated_inputs() {
    let mut rng = XorShift64(0x5eed_cafe_d00d_0001);
    let seed_json = br#"{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.test/v1/items/1"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"id\":1}"}}}]}}"#;
    let limits = ImportLimits {
        default_max_bytes: 64 * 1024,
        hard_max_bytes: 64 * 1024,
        max_entries: 128,
        max_json_depth: 16,
        tolerate_partial_entries: true,
    };

    for iter in 0..250 {
        let mut buf = seed_json.to_vec();
        let mode = iter % 4;
        match mode {
            0 => {
                let cut = rng.next_range(buf.len().max(1));
                buf.truncate(cut);
            }
            1 => {
                let idx = rng.next_range(buf.len());
                buf[idx] = (rng.next_u64() & 0xff) as u8;
            }
            2 => {
                let depth = 1 + rng.next_range(28);
                let mut nested = vec![b'{'; depth];
                nested.extend(vec![b'}'; depth]);
                buf = nested;
            }
            _ => {
                let extra = 1 + rng.next_range(16);
                for _ in 0..extra {
                    let pos = rng.next_range(buf.len() + 1);
                    buf.insert(pos, b'"');
                }
            }
        }

        let _ = stream_har_reader(&buf[..], &limits, |_idx, _entry| Ok(()));
    }
}

#[test]
fn property_normalizer_preserves_order_and_never_leaks_synthetic_secrets() {
    let mut rng = XorShift64(0x5eed_cafe_d00d_0001);

    for iter in 0..250 {
        let q_count = 1 + rng.next_range(12);
        let mut query_objs = Vec::with_capacity(q_count);
        let mut expected_keys = Vec::with_capacity(q_count);
        for q in 0..q_count {
            let key = if q % 3 == 0 {
                "dup_tag".to_string()
            } else {
                format!("param_{q}")
            };
            let val = format!("val_{iter}_{q}");
            expected_keys.push((key.clone(), val.clone()));
            query_objs.push(format!(r#"{{"name":"{key}","value":"{val}"}}"#));
        }

        let hdr_dup_count = 1 + rng.next_range(4);
        let secret_bearer = format!("hf_synthetic_prop_bearer_{iter}");
        let secret_cookie = format!("hf_synthetic_prop_cookie_{iter}");
        let secret_pw = format!("hf_synthetic_prop_pw_{iter}");
        let secret_resp_tok = format!("hf_synthetic_prop_resp_{iter}");

        let mut hdr_objs: Vec<String> = (0..hdr_dup_count)
            .map(|h| format!(r#"{{"name":"X-Dup","value":"h_{h}"}}"#))
            .collect();
        hdr_objs.push(format!(
            r#"{{"name":"Authorization","value":"Bearer {secret_bearer}"}}"#
        ));
        hdr_objs.push(format!(
            r#"{{"name":"Cookie","value":"sid={secret_cookie}"}}"#
        ));

        let resp_json_raw = format!(r#"{{"access_token":"{secret_resp_tok}","iter":{iter}}}"#);
        let resp_b64 = STANDARD.encode(resp_json_raw.as_bytes());
        let get_body =
            format!(r#"{{"password":"{secret_pw}","probe":{iter}}}"#).replace('"', "\\\"");

        let har_doc = format!(
            r#"{{
              "log": {{
                "version": "1.2",
                "entries": [
                  {{
                    "request": {{
                      "method": "GET",
                      "url": "https://api.example.test/v1/items/{iter}",
                      "headers": [{headers}],
                      "queryString": [{queries}],
                      "postData": {{
                        "mimeType": "application/json",
                        "text": "{get_body}"
                      }}
                    }},
                    "response": {{
                      "status": 200,
                      "headers": [
                        {{"name":"Set-Cookie","value":"sid={secret_cookie}; HttpOnly"}}
                      ],
                      "content": {{
                        "mimeType": "application/json",
                        "encoding": "base64",
                        "text": "{resp_b64}"
                      }}
                    }}
                  }}
                ]
              }}
            }}"#,
            headers = hdr_objs.join(","),
            queries = query_objs.join(","),
        );

        let (session, summary) = normalize_har_reader(
            "prop-sess",
            "prop.har",
            har_doc.as_bytes(),
            &ImportLimits::default(),
        )
        .expect("normalize property har");

        assert_eq!(summary.processed_entries, 1);
        let entry = &session.entries[0];
        assert_eq!(entry.method, "GET");
        assert!(entry.request_body.is_some());
        assert_eq!(entry.query_params, expected_keys);
        assert_eq!(entry.request_headers.len(), hdr_dup_count + 2);
        assert_eq!(entry.request_cookies[0].1, entry.response_cookies[0].1);

        let serialized = serde_json::to_string(&session).expect("serialize session");
        for secret in [&secret_bearer, &secret_cookie, &secret_pw, &secret_resp_tok] {
            assert!(
                !serialized.contains(secret),
                "secret '{secret}' leaked on iteration {iter}"
            );
        }
    }
}
