use reqwest::{Client, Method, Response, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
/**
* Flutterwave HTTP (V3) Implementation.
*/

pub const FLW_BASE_URL: &'static str = "https://api.flutterwave.com/v3/";

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkData {
    pub link: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentLinkResponse {
    pub status: String,
    pub message: String,
    pub data: LinkData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentRequestData {
    pub tx_ref: String,
    pub amount: f64,
    pub customer: CustomerData,
    pub customizations: Customizations,
    pub description: String,
    pub redirect_url: String,
    pub currency: String,
}

impl PaymentRequestData {
    pub fn new(
        tx_ref: String,
        amount: f64,
        customer_email: String,
        customer_name: String,
        customer_phone: String,
        description: String,
    ) -> Self {
        let hostname =
            std::env::var("HOSTNAME").unwrap_or_else(|_| "http://localhost:8000".to_string());
        let redirect_url = format!("{}/api/flutterwave-callback", hostname);
        let customer = CustomerData::new(customer_name, customer_phone, customer_email);
        let customizations = Customizations::new(
            &description,
            &"https://robia.ug/static/svg/logo.svg".to_string(),
        );

        PaymentRequestData {
            tx_ref,
            amount,
            description,
            redirect_url,
            customer,
            customizations,
            currency: "UGX".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customizations {
    title: String,
    logo: String,
}

impl Customizations {
    pub fn new(title: &String, logo: &String) -> Self {
        return Customizations {
            title: title.to_string(),
            logo: logo.to_string(),
        };
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomerData {
    pub name: String,
    pub phone_number: String,
    pub email: String,
}

impl CustomerData {
    pub fn new(name: String, phone_number: String, email: String) -> Self {
        CustomerData {
            name,
            phone_number,
            email,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedPaymentResponseData {
    pub id: u64,
    pub tx_ref: String,
    pub flw_ref: String,
    pub device_fingerprint: String,
    pub amount: u64,
    pub currency: String,
    pub charged_amount: u64,
    pub app_fee: u64,
    pub merchant_fee: u64,
    pub processor_response: String,
    pub auth_model: String,
    pub ip: String,
    pub narration: String,
    pub status: String,
    pub payment_type: String,
    pub created_at: String,
    pub account_id: u64,
    pub amount_settled: u64,
    pub customer: CustomerData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedPaymentResponse {
    pub status: String,
    pub message: String,
    pub data: VerifiedPaymentResponseData,
}

#[derive(Debug, Serialize)]
pub struct EmptyPayload();

#[derive(Debug, Clone)]
pub struct Flutterwave(Option<Client>);

#[derive(Debug, Serialize, Deserialize)]
pub struct FlutterwaveError(pub String);

impl Flutterwave {
    /// Create a new flutterwave instance. Panics if the environment variable
    /// FLW_CLIENT_SECRET is not set.
    pub async fn new() -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        let flw_secret = std::env::var("FLW_CLIENT_SECRET").unwrap();
        let mut auth_value =
            header::HeaderValue::from_str(&format!("Bearer {}", &flw_secret)).unwrap();
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        Flutterwave(Some(client))
    }
    /// Make a request to the flutterwave API using a specified method
    pub async fn _request<T: Serialize + ?Sized>(
        self,
        path: &String,
        method: Method,
        json_body: Option<&T>,
    ) -> Result<Response, FlutterwaveError>
    where
        T: Serialize,
    {
        match self.0 {
            Some(client) => {
                let url = format!("{}{}", FLW_BASE_URL, path);
                let mut response = client.request(method, url);
                match json_body {
                    Some(data) => {
                        response = response.json(&json!(data));
                    }
                    None => {}
                };
                Ok(response
                    .send()
                    .await
                    .map_err(|e| FlutterwaveError(format!("request error: {}", e.to_string())))
                    .unwrap())
            }
            None => {
                return Err(FlutterwaveError(
                    "HTTP Client unavailable for requests".to_string(),
                ));
            }
        }
    }
    /// Get a payment link from flutterwave
    pub async fn get_payment_link(
        self,
        payment_data: &PaymentRequestData,
    ) -> Result<PaymentLinkResponse, FlutterwaveError> {
        match self
            ._request::<PaymentRequestData>(
                &"payments".to_string(),
                Method::POST,
                Some(payment_data),
            )
            .await
        {
            Ok(response) => {
                let text = response.text().await.unwrap();
                match serde_json::from_str::<PaymentLinkResponse>(&text) {
                    Ok(link_reponse) => return Ok(link_reponse),
                    Err(e) => {
                        log::error!("Json error {}", e.to_string());
                        return Err(FlutterwaveError(e.to_string()));
                    }
                };
            }
            Err(e) => Err(e),
        }
    }
    /// Verifies the status of a payment using its transaction reference.
    pub async fn verify_payment(
        self,
        tx_ref: &String,
    ) -> Result<VerifiedPaymentResponse, FlutterwaveError> {
        let url = format!("transactions/verify_by_reference?tx_ref={}", tx_ref);
        match self._request::<EmptyPayload>(&url, Method::GET, None).await {
            Ok(response) => {
                let text = response.text().await.unwrap();
                match serde_json::from_str::<VerifiedPaymentResponse>(&text) {
                    Ok(payment_reponse) => return Ok(payment_reponse),
                    Err(e) => {
                        log::error!("Json error {}", e.to_string());
                        return Err(FlutterwaveError(e.to_string()));
                    }
                };
            }
            Err(e) => Err(e),
        }
    }
}
