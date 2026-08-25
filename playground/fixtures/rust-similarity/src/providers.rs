pub struct ProviderRequest {
    pub endpoint: String,
    pub token: String,
}

pub fn adapt_atlas(request: &ProviderRequest) -> String {
    let mut headers = Vec::new();
    headers.push(("authorization", request.token.as_str()));
    if request.endpoint.starts_with("https://") {
        headers.push(("transport", "secure"));
    } else {
        headers.push(("transport", "local"));
    }
    format!("atlas:{}:{}", request.endpoint, headers.len())
}

pub fn adapt_boreal(input: &ProviderRequest) -> String {
    let mut metadata = Vec::new();
    metadata.push(("authorization", input.token.as_str()));
    if input.endpoint.starts_with("https://") {
        metadata.push(("transport", "secure"));
    } else {
        metadata.push(("transport", "local"));
    }
    format!("boreal:{}:{}", input.endpoint, metadata.len())
}

pub fn adapt_cascade(payload: &ProviderRequest) -> String {
    let mut fields = Vec::new();
    fields.push(("authorization", payload.token.as_str()));
    if payload.endpoint.starts_with("https://") {
        fields.push(("transport", "secure"));
    } else {
        fields.push(("transport", "local"));
    }
    format!("cascade:{}:{}", payload.endpoint, fields.len())
}
