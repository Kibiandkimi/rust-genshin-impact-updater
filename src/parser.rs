use serde::Deserialize;

mod u64_string {
    use serde::{Deserialize, Deserializer};
    use std::fmt;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U64Visitor;

        impl<'de> serde::de::Visitor<'de> for U64Visitor {
            type Value = u64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing a u64")
            }

            fn visit_str<E>(self, value: &str) -> Result<u64, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u64>().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(U64Visitor)
    }
}

#[derive(Debug, Deserialize)]
pub struct Response {
    pub retcode: i32,
    pub message: String,
    pub data: Data,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    #[serde(rename = "game_packages")]
    pub game_packages: Vec<GamePackage>,
}

#[derive(Debug, Deserialize)]
pub struct GamePackage {
    pub game: Game,
    pub main: Main,
    #[serde(rename = "pre_download")]
    pub pre_download: PreDownload,
}

#[derive(Debug, Deserialize)]
pub struct Game {
    pub id: String,
    pub biz: String,
}

#[derive(Debug, Deserialize)]
pub struct Main {
    pub major: Major,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Deserialize)]
pub struct Major {
    pub version: String,
    #[serde(rename = "game_pkgs")]
    pub game_pkgs: Vec<GamePkg>,
    #[serde(rename = "audio_pkgs")]
    pub audio_pkgs: Vec<AudioPkg>,
    #[serde(rename = "res_list_url")]
    pub res_list_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GamePkg {
    pub url: String,
    pub md5: String,
    #[serde(deserialize_with = "u64_string::deserialize")]
    pub size: u64,
    #[serde(rename = "decompressed_size", deserialize_with = "u64_string::deserialize")]
    pub decompressed_size: u64,
}

#[derive(Debug, Deserialize)]
pub struct AudioPkg {
    pub language: String,
    pub url: String,
    pub md5: String,
    #[serde(deserialize_with = "u64_string::deserialize")]
    pub size: u64,
    #[serde(rename = "decompressed_size", deserialize_with = "u64_string::deserialize")]
    pub decompressed_size: u64,
}

#[derive(Debug, Deserialize)]
pub struct Patch {
    pub version: String,
    #[serde(rename = "game_pkgs")]
    pub game_pkgs: Vec<GamePkg>,
    #[serde(rename = "audio_pkgs")]
    pub audio_pkgs: Vec<AudioPkg>,
    #[serde(rename = "res_list_url")]
    pub res_list_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PreDownload {
    pub major: Option<Major>,
    pub patches: Vec<Patch>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u64_string_deserialization() {
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "u64_string::deserialize")]
            value: u64,
        }

        let json = r#"{"value": "123456"}"#;
        let test: Test = serde_json::from_str(json).unwrap();
        assert_eq!(test.value, 123456);
    }

    #[test]
    fn test_full_response_parsing() {
        let sample_response = r#"
        {
            "retcode": 0,
            "message": "success",
            "data": {
                "game_packages": [
                    {
                        "game": {
                            "id": "gopR6Cufr3",
                            "biz": "test_biz"
                        },
                        "main": {
                            "major": {
                                "version": "1.0.0",
                                "game_pkgs": [
                                    {
                                        "url": "http://example.com/game",
                                        "md5": "abcd1234",
                                        "size": "1024",
                                        "decompressed_size": "2048"
                                    }
                                ],
                                "audio_pkgs": [],
                                "res_list_url": ""
                            },
                            "patches": [
                                {
                                    "version": "1.0.1",
                                    "game_pkgs": [],
                                    "audio_pkgs": [],
                                    "res_list_url": ""
                                }
                            ]
                        },
                        "pre_download": {
                            "major": null,
                            "patches": []
                        }
                    }
                ]
            }
        }"#;

        let response: Response = serde_json::from_str(sample_response).unwrap();
        assert_eq!(response.retcode, 0);
        assert_eq!(response.data.game_packages[0].game.id, "gopR6Cufr3");
    }
}