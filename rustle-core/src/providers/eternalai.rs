//! EternalAI API client and rustle integration
//!
//! # Example
//! ```
//! use rustle::providers::eternalai;
//!
//! let client = eternalai::Client::new("YOUR_API_KEY");
//!
//! let gpt4o = client.completion_model(eternalai::NOUS_RESErustleH_HERMES_3_LLAMA_3_1_70B_FP8);
//! ```

use crate::{
    agent::AgentBuilder,
    completion::{self, CompletionError, CompletionRequest},
    embeddings::{self, EmbeddingError, EmbeddingsBuilder},
    extractor::ExtractorBuilder,
    json_utils, Embed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

// ================================================================
// Main EternalAI Client
// ================================================================
const ETERNALAI_API_BASE_URL: &str = "https://api.eternalai.org/v1";

#[derive(Clone)]
pub struct Client {
    base_url: String,
    http_client: reqwest::Client,
}

impl Client {
    /// Create a new EternalAI client with the given API key.
    pub fn new(api_key: &str) -> Self {
        Self::from_url(api_key, ETERNALAI_API_BASE_URL)
    }

    /// Create a new EternalAI client with the given API key and base API URL.
    pub fn from_url(api_key: &str, base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::builder()
                .default_headers({
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        "Authorization",
                        format!("Bearer {}", api_key)
                            .parse()
                            .expect("Bearer token should parse"),
                    );
                    headers
                })
                .timeout(Duration::from_secs(120))
                .build()
                .expect("EternalAI reqwest client should build"),
        }
    }

    /// Create a new EternalAI client from the `ETERNALAI_API_KEY` environment variable.
    /// Panics if the environment variable is not set.
    pub fn from_env() -> Self {
        let api_key = std::env::var("ETERNALAI_API_KEY").expect("ETERNALAI_API_KEY not set");
        Self::new(&api_key)
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/{}", self.base_url, path).replace("//", "/");
        self.http_client.post(url)
    }

    /// Create an embedding model with the given name.
    /// Note: default embedding dimension of 0 will be used if model is not known.
    /// If this is the case, it's better to use function `embedding_model_with_ndims`
    ///
    /// # Example
    /// ```
    /// use rustle::providers::eternalai::{Client, self};
    ///
    /// // Initialize the EternalAI client
    /// let eternalai = Client::new("your-open-ai-api-key");
    ///
    /// let embedding_model = eternalai.embedding_model(eternalai::TEXT_EMBEDDING_3_LARGE);
    /// ```
    pub fn embedding_model(&self, model: &str) -> EmbeddingModel {
        let ndims = match model {
            TEXT_EMBEDDING_3_LARGE => 3072,
            TEXT_EMBEDDING_3_SMALL | TEXT_EMBEDDING_ADA_002 => 1536,
            _ => 0,
        };
        EmbeddingModel::new(self.clone(), model, ndims)
    }

    /// Create an embedding model with the given name and the number of dimensions in the embedding generated by the model.
    ///
    /// # Example
    /// ```
    /// use rustle::providers::eternalai::{Client, self};
    ///
    /// // Initialize the EternalAI client
    /// let eternalai = Client::new("your-open-ai-api-key");
    ///
    /// let embedding_model = eternalai.embedding_model("model-unknown-to-rustle", 3072);
    /// ```
    pub fn embedding_model_with_ndims(&self, model: &str, ndims: usize) -> EmbeddingModel {
        EmbeddingModel::new(self.clone(), model, ndims)
    }

    /// Create an embedding builder with the given embedding model.
    ///
    /// # Example
    /// ```
    /// use rustle::providers::eternalai::{Client, self};
    ///
    /// // Initialize the EternalAI client
    /// let eternalai = Client::new("your-open-ai-api-key");
    ///
    /// let embeddings = eternalai.embeddings(eternalai::TEXT_EMBEDDING_3_LARGE)
    ///     .simple_document("doc0", "Hello, world!")
    ///     .simple_document("doc1", "Goodbye, world!")
    ///     .build()
    ///     .await
    ///     .expect("Failed to embed documents");
    /// ```
    pub fn embeddings<D: Embed>(&self, model: &str) -> EmbeddingsBuilder<EmbeddingModel, D> {
        EmbeddingsBuilder::new(self.embedding_model(model))
    }

    /// Create a completion model with the given name.
    ///
    /// # Example
    /// ```
    /// use rustle::providers::eternalai::{Client, self};
    ///
    /// // Initialize the EternalAI client
    /// let eternalai = Client::new("your-open-ai-api-key");
    ///
    /// let gpt4 = eternalai.completion_model(eternalai::GPT_4);
    /// ```
    pub fn completion_model(&self, model: &str, chain_id: Option<&str>) -> CompletionModel {
        CompletionModel::new(self.clone(), model, chain_id)
    }

    /// Create an agent builder with the given completion model.
    ///
    /// # Example
    /// ```
    /// use rustle::providers::eternalai::{Client, self};
    ///
    /// // Initialize the Eternal client
    /// let eternalai = Client::new("your-open-ai-api-key");
    ///
    /// let agent = eternalai.agent(eternalai::UNSLOTH_LLAMA_3_3_70B_INSTRUCT_BNB_4BIT, None)
    ///    .preamble("You are comedian AI with a mission to make people laugh.")
    ///    .temperature(0.0)
    ///    .build();
    /// ```
    pub fn agent(&self, model: &str, chain_id: Option<&str>) -> AgentBuilder<CompletionModel> {
        AgentBuilder::new(self.completion_model(model, chain_id))
    }

    /// Create an extractor builder with the given completion model.
    pub fn extractor<T: JsonSchema + for<'a> Deserialize<'a> + Serialize + Send + Sync>(
        &self,
        model: &str,
    ) -> ExtractorBuilder<T, CompletionModel> {
        ExtractorBuilder::new(self.completion_model(model, None))
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiResponse<T> {
    Ok(T),
    Err(ApiErrorResponse),
}

// ================================================================
// EternalAI Embedding API
// ================================================================
/// `text-embedding-3-large` embedding model
pub const TEXT_EMBEDDING_3_LARGE: &str = "text-embedding-3-large";
/// `text-embedding-3-small` embedding model
pub const TEXT_EMBEDDING_3_SMALL: &str = "text-embedding-3-small";
/// `text-embedding-ada-002` embedding model
pub const TEXT_EMBEDDING_ADA_002: &str = "text-embedding-ada-002";

#[derive(Debug, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: Usage,
}

impl From<ApiErrorResponse> for EmbeddingError {
    fn from(err: ApiErrorResponse) -> Self {
        EmbeddingError::ProviderError(err.message)
    }
}

impl From<ApiResponse<EmbeddingResponse>> for Result<EmbeddingResponse, EmbeddingError> {
    fn from(value: ApiResponse<EmbeddingResponse>) -> Self {
        match value {
            ApiResponse::Ok(response) => Ok(response),
            ApiResponse::Err(err) => Err(EmbeddingError::ProviderError(err.message)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f64>,
    pub index: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Prompt tokens: {} Total tokens: {}",
            self.prompt_tokens, self.total_tokens
        )
    }
}

#[derive(Clone)]
pub struct EmbeddingModel {
    client: Client,
    pub model: String,
    ndims: usize,
}

impl embeddings::EmbeddingModel for EmbeddingModel {
    const MAX_DOCUMENTS: usize = 1024;

    fn ndims(&self) -> usize {
        self.ndims
    }

    async fn embed_texts(
        &self,
        documents: impl IntoIterator<Item = String>,
    ) -> Result<Vec<embeddings::Embedding>, EmbeddingError> {
        let documents = documents.into_iter().collect::<Vec<_>>();

        let response = self
            .client
            .post("/embeddings")
            .json(&json!({
                "model": self.model,
                "input": documents,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<ApiResponse<EmbeddingResponse>>().await? {
                ApiResponse::Ok(response) => {
                    tracing::info!(target: "rustle",
                        "EternalAI embedding token usage: {}",
                        response.usage
                    );

                    if response.data.len() != documents.len() {
                        return Err(EmbeddingError::ResponseError(
                            "Response data length does not match input length".into(),
                        ));
                    }

                    Ok(response
                        .data
                        .into_iter()
                        .zip(documents.into_iter())
                        .map(|(embedding, document)| embeddings::Embedding {
                            document,
                            vec: embedding.embedding,
                        })
                        .collect())
                }
                ApiResponse::Err(err) => Err(EmbeddingError::ProviderError(err.message)),
            }
        } else {
            Err(EmbeddingError::ProviderError(response.text().await?))
        }
    }
}

impl EmbeddingModel {
    pub fn new(client: Client, model: &str, ndims: usize) -> Self {
        Self {
            client,
            model: model.to_string(),
            ndims,
        }
    }
}

// ================================================================
// EternalAI Completion API
// ================================================================
pub const NOUS_RESErustleH_HERMES_3_LLAMA_3_1_70B_FP8: &str =
    "NousReserustleh/Hermes-3-Llama-3.1-70B-FP8";
pub const UNSLOTH_LLAMA_3_3_70B_INSTRUCT_BNB_4BIT: &str = "unsloth/Llama-3.3-70B-Instruct-bnb-4bit";

pub const MAPPING_CHAINID: [(&str, &str); 2] = [
    (NOUS_RESErustleH_HERMES_3_LLAMA_3_1_70B_FP8, "45762"),
    (UNSLOTH_LLAMA_3_3_70B_INSTRUCT_BNB_4BIT, "45762"),
];

pub fn get_chain_id(key: &str) -> Option<&str> {
    for &(k, v) in &MAPPING_CHAINID {
        if k == key {
            return Some(v);
        }
    }
    None
}

#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
    pub onchain_data: Option<Value>,
}

impl From<ApiErrorResponse> for CompletionError {
    fn from(err: ApiErrorResponse) -> Self {
        CompletionError::ProviderError(err.message)
    }
}

impl TryFrom<CompletionResponse> for completion::CompletionResponse<CompletionResponse> {
    type Error = CompletionError;

    fn try_from(value: CompletionResponse) -> std::prelude::v1::Result<Self, Self::Error> {
        match value.choices.as_slice() {
            [Choice {
                message:
                    Message {
                        tool_calls: Some(calls),
                        ..
                    },
                ..
            }, ..] => {
                let call = calls.first().ok_or(CompletionError::ResponseError(
                    "Tool selection is empty".into(),
                ))?;

                Ok(completion::CompletionResponse {
                    choice: completion::ModelChoice::ToolCall(
                        call.function.name.clone(),
                        serde_json::from_str(&call.function.arguments)?,
                    ),
                    raw_response: value,
                })
            }
            [Choice {
                message:
                    Message {
                        content: Some(content),
                        ..
                    },
                ..
            }, ..] => Ok(completion::CompletionResponse {
                choice: completion::ModelChoice::Message(content.to_string()),
                raw_response: value,
            }),
            _ => Err(CompletionError::ResponseError(
                "Response did not contain a message or tool call".into(),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: Message,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: Function,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: completion::ToolDefinition,
}

impl From<completion::ToolDefinition> for ToolDefinition {
    fn from(tool: completion::ToolDefinition) -> Self {
        Self {
            r#type: "function".into(),
            function: tool,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Function {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone)]
pub struct CompletionModel {
    client: Client,
    /// Name of the model (e.g.: gpt-3.5-turbo-1106)
    pub model: String,
    pub chain_id: String,
}

impl CompletionModel {
    pub fn new(client: Client, model: &str, chain_id: Option<&str>) -> Self {
        Self {
            client,
            model: model.to_string(),
            chain_id: chain_id.unwrap_or("").to_string(),
        }
    }
}

impl completion::CompletionModel for CompletionModel {
    type Response = CompletionResponse;

    async fn completion(
        &self,
        mut completion_request: CompletionRequest,
    ) -> Result<completion::CompletionResponse<CompletionResponse>, CompletionError> {
        // Add preamble to chat history (if available)
        let mut full_history = if let Some(preamble) = &completion_request.preamble {
            vec![completion::Message {
                role: "system".into(),
                content: preamble.clone(),
            }]
        } else {
            vec![]
        };

        // Extend existing chat history
        full_history.append(&mut completion_request.chat_history);

        // Add context documents to chat history
        let prompt_with_context = completion_request.prompt_with_context();

        // Add context documents to chat history
        full_history.push(completion::Message {
            role: "user".into(),
            content: prompt_with_context,
        });

        let mut chain_id = self.chain_id.clone();
        if chain_id.is_empty() {
            chain_id = get_chain_id(self.model.as_str()).unwrap_or("").to_string();
        }

        let request = if completion_request.tools.is_empty() {
            json!({
                "model": self.model,
                "chain_id": chain_id,
                "messages": full_history,
                "temperature": completion_request.temperature,
            })
        } else {
            json!({
                "model": self.model,
                "chain_id": chain_id,
                "messages": full_history,
                "temperature": completion_request.temperature,
                "tools": completion_request.tools.into_iter().map(ToolDefinition::from).collect::<Vec<_>>(),
                "tool_choice": "auto",
            })
        };

        let response = self
            .client
            .post("/chat/completions")
            .json(
                &if let Some(params) = completion_request.additional_params {
                    json_utils::merge(request, params)
                } else {
                    request
                },
            )
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<ApiResponse<CompletionResponse>>().await? {
                ApiResponse::Ok(response) => {
                    tracing::info!(target: "rustle",
                        "EternalAI completion token usage: {:?}",
                        response.usage.clone().map(|usage| format!("{usage}")).unwrap_or("N/A".to_string())
                    );
                    match &response.onchain_data {
                        Some(data) => {
                            let onchain_data = serde_json::to_string_pretty(data)?;
                            println!("onchain_data: {}", onchain_data);
                        }
                        None => {
                            println!("onchain_data: None");
                        }
                    }
                    response.try_into()
                }
                ApiResponse::Err(err) => Err(CompletionError::ProviderError(err.message)),
            }
        } else {
            Err(CompletionError::ProviderError(response.text().await?))
        }
    }
}
