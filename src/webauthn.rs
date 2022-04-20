use url::Url;
use webauthn_rs::error::WebauthnError;
use webauthn_rs::proto::{
    AttestationConveyancePreference, AuthenticatorAttachment, COSEAlgorithm,
    CreationChallengeResponse, Credential, CredentialID, RegisterPublicKeyCredential,
};
use webauthn_rs::{Webauthn, WebauthnConfig};

type WebauthnResult<T> = core::result::Result<T, WebauthnError>;

use crate::db::{User, DB};

pub struct WebauthnVolatileConfig {
    pub rp_name: String,
    pub rp_id: String,
    pub rp_origin: Url,
    pub attachment: Option<AuthenticatorAttachment>,
}

impl WebauthnConfig for WebauthnVolatileConfig {
    /// Returns the relying party name. See the trait documentation for more.
    fn get_relying_party_name(&self) -> &str {
        &self.rp_name
    }

    /// Returns the relying party id. See the trait documentation for more.
    fn get_relying_party_id(&self) -> &str {
        &self.rp_id
    }

    /// Retrieve the relying party origin. See the trait documentation for more.
    fn get_origin(&self) -> &Url {
        &self.rp_origin
    }

    /// Retrieve the authenticator attachment hint. See the trait documentation for more.
    fn get_authenticator_attachment(&self) -> Option<AuthenticatorAttachment> {
        self.attachment
    }

    /// Retrieve the authenticator attestation preference. See the trait documentation for more.
    fn get_attestation_preference(&self) -> AttestationConveyancePreference {
        AttestationConveyancePreference::Direct
    }

    /// Retrieve the list of support algorithms.
    ///
    /// WARNING: This returns *all* possible algorithms, not just SUPPORTED ones. This
    /// is so that
    fn get_credential_algorithms(&self) -> Vec<COSEAlgorithm> {
        vec![
            COSEAlgorithm::ES256,
            COSEAlgorithm::ES384,
            COSEAlgorithm::ES512,
            COSEAlgorithm::RS256,
            COSEAlgorithm::RS384,
            COSEAlgorithm::RS512,
            COSEAlgorithm::PS256,
            COSEAlgorithm::PS384,
            COSEAlgorithm::PS512,
            COSEAlgorithm::EDDSA,
        ]
    }

    /// Allow subdomains
    fn allow_subdomains_origin(&self) -> bool {
        true
    }
}

impl WebauthnVolatileConfig {
    /// Create a new Webauthn Ephemeral instance. This requires a provided relying party
    /// name, origin and id. See the trait documentation for more detail on relying party
    /// name, origin and id.
    pub fn new(
        rp_name: &str,
        rp_origin: &str,
        rp_id: &str,
        attachment: Option<AuthenticatorAttachment>,
    ) -> Self {
        dbg!(rp_origin);
        WebauthnVolatileConfig {
            rp_name: rp_name.to_string(),
            rp_id: rp_id.to_string(),
            rp_origin: Url::parse(rp_origin).expect("Failed to parse RP origin"),
            attachment,
        }
    }
}

pub struct WebauthnActor {
    wan: Webauthn<WebauthnVolatileConfig>,
}

impl WebauthnActor {
    pub fn new(config: WebauthnVolatileConfig) -> Self {
        WebauthnActor {
            wan: Webauthn::new(config),
        }
    }

    pub async fn challenge_register(
        &self,
        db: &mut DB,
        username: &String,
    ) -> WebauthnResult<CreationChallengeResponse> {
        println!("handle challenge_register -> {:?}", &username);
        let user: User = match db.get_user(username).await {
            Ok(user) => user,
            Err(_) => return Err(WebauthnError::UserNotPresent),
        };
        let excluded: Option<Vec<CredentialID>> = if user.webauthn_credentials.len() > 0 {
            Some(
                user.webauthn_credentials
                    .iter()
                    .map(|cred| cred.cred_id.clone())
                    .collect(),
            )
        } else {
            Option::default()
        };
        let (ccr, rs) = self.wan.generate_challenge_register_options(
            username.as_bytes().to_vec(),
            username.clone(),
            username.clone(),
            excluded,
            Some(webauthn_rs::proto::UserVerificationPolicy::Discouraged),
            None,
        )?;
        match db.save_webauthn_registration_state(&username, &rs).await {
            Ok(()) => (),
            Err(_) => return Err(WebauthnError::ChallengePersistenceError),
        }
        println!("complete challenge_register -> {:?}", &ccr);
        Ok(ccr)
    }

    pub async fn register(
        &self,
        db: &mut DB,
        username: &String,
        reg: &RegisterPublicKeyCredential,
    ) -> WebauthnResult<()> {
        println!(
            "handle register -> (username: {:?}, reg: {:?})",
            username, reg
        );
        let user = match db.get_user(&username).await {
            Ok(user) => user,
            Err(_) => return Err(WebauthnError::UserNotPresent),
        };
        let rs = match user.webauthn_registration_state {
            Some(rs) => rs,
            None => return Err(WebauthnError::ChallengeNotFound),
        };
        let mut ucreds: Vec<Credential> = user.webauthn_credentials;
        match self
            .wan
            .register_credential(reg, &rs, |cred_id| {
                dbg!(&cred_id);
                Ok(false)
            })
            .map(|cred| {
                ucreds.push(cred.0);
            }) {
            Ok(()) => (),
            Err(e) => println!("Error: {:?}", e),
        }
        match db.save_webauthn_registration(username, &ucreds).await {
            Ok(()) => (),
            Err(e) => println!("Error: {:?}", e),
        }
        println!("complete register");
        Ok(())
    }

    /*
    pub async fn challenge_authenticate(
        &self,
        user: &mut User,
    ) -> WebauthnResult<RequestChallengeResponse> {
        println!("handle challenge_authenticate -> {:?}", &user.username);

        let creds = match self
            .creds
            .lock()
            .await
            .get(&user.username.as_bytes().to_vec())
        {
            Some(creds) => Some(creds.iter().map(|(_, v)| v.clone()).collect()),
            None => None,
        }
        .ok_or(WebauthnError::CredentialRetrievalError)?;

        let exts = RequestAuthenticationExtensions::builder()
            .get_cred_blob(true)
            .build();

        let (acr, st) = self
            .wan
            .generate_challenge_authenticate_options(creds, Some(exts))?;
        self.auth_chals
            .lock()
            .await
            .put(user.username.as_bytes().to_vec(), st);
        println!("complete challenge_authenticate -> {:?}", &acr);
        Ok(acr)
    }
    */

    /*
    pub async fn authenticate(
        &self,
        username: &String,
        lgn: &PublicKeyCredential,
    ) -> WebauthnResult<()> {
        println!(
            "handle authenticate -> (username: {:?}, lgn: {:?})",
            username,
            lgn
        );
        let username = username.as_bytes().to_vec();
        let st = self
            .auth_chals
            .lock()
            .await
            .pop(&username)
            .ok_or(WebauthnError::ChallengeNotFound)?;
        let mut creds = self.creds.lock().await;
        let r = self
            .wan
            .authenticate_credential(lgn, &st)
            .map(|(cred_id, auth_data)| {
                let _ = match creds.get_mut(&username) {
                    Some(v) => {
                        let mut c = v.remove(cred_id).unwrap();
                        c.counter = auth_data.counter;
                        v.insert(cred_id.clone(), c);
                        Ok(())
                    }
                    None => {
                        // Invalid state but not end of world ...
                        Err(())
                    }
                };
                ()
            });
        println!("complete authenticate -> {:?}", &r);
        r
    }
    */
}
