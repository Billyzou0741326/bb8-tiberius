use azure_core::credentials::TokenCredential as _;
use bb8_tiberius::IntoConfig as _;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn_str = std::env::var("DB_CONN")?;
    let tenant_id = std::env::var("TENANT_ID")?;
    let client_id = std::env::var("CLIENT_ID")?;
    let client_secret = azure_core::credentials::Secret::from(std::env::var("CLIENT_SECRET")?);

    let client_secret_credential = azure_identity::ClientSecretCredential::new(
        tenant_id.as_str(),
        client_id,
        client_secret,
        None,
    )?;
    let aad_token_client = AADTokenClient::new(client_secret_credential);

    let config = conn_str.as_str().into_config()?;
    let mgr =
        bb8_tiberius::ConnectionManager::new(config).with_aad_token_provider(aad_token_client);
    let pool = bb8::Pool::builder().max_size(2).build_unchecked(mgr);
    let mut conn = pool.get().await?;

    let res = conn
        .simple_query("SELECT @@version")
        .await?
        .into_first_result()
        .await?
        .into_iter()
        .map(|row| {
            let val: &str = row.get(0).unwrap();
            String::from(val)
        })
        .collect::<Vec<_>>();

    println!("{:?}", &res);

    Ok(())
}

struct AADTokenClient {
    client_secret_credential: std::sync::Arc<azure_identity::ClientSecretCredential>,
}

impl AADTokenClient {
    pub fn new(
        client_secret_credential: std::sync::Arc<azure_identity::ClientSecretCredential>,
    ) -> Self {
        Self {
            client_secret_credential,
        }
    }
}

impl bb8_tiberius::AADTokenProvider for AADTokenClient {
    fn get_aad_token(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<String, Box<dyn std::error::Error + Send + 'static>>,
                > + Send
                + 'static,
        >,
    > {
        let client_secret_credential = self.client_secret_credential.clone();
        Box::pin(async move {
            match client_secret_credential
                .get_token(&["https://database.windows.net//.default"], None)
                .await
            {
                Ok(token) => Ok(token.token.secret().to_owned()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + 'static>),
            }
        })
    }
}
