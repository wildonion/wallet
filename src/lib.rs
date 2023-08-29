

use sha2::{Digest, Sha256};
use std::fmt::Write;
use ring::{signature as ring_signature, rand as ring_rand};
use ring::signature::Ed25519KeyPair;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use ring::{signature::KeyPair, pkcs8::Document};
use secp256k1::Secp256k1;
use secp256k1::ecdsa::Signature;
use secp256k1::{rand::SeedableRng, rand::rngs::StdRng, PublicKey, SecretKey, Message, hashes::sha256};
use std::io::BufWriter;
use tiny_keccak::keccak256;
use std::str::FromStr;
use std::{fs::OpenOptions, io::BufReader};
use web3::{
    transports,
    types::{Address, TransactionParameters, H256, U256},
    Web3,
};
use themis::keys as themis_keys;
use themis::secure_message::{SecureSign, SecureVerify};
use themis::keygen::gen_ec_key_pair;
use themis::keys::{EcdsaKeyPair, EcdsaPrivateKey, EcdsaPublicKey};
use themis::keys::KeyPair as ThemisKeyPair;
use secp256k1::hashes::Hash;
use rand::random;


pub fn string_to_static_str(s: String) -> &'static str { 
    Box::leak(s.into_boxed_str()) 
}




/* 
     ---------------------------------------------------------------------
    |  RSA (Asymmetric) Crypto Wallet Implementations using ECC Algorithms
    |---------------------------------------------------------------------
    | ed25519   -> EdDSA 
    | secp256k1 -> EC (can be imported in EVM based wallets like metamask)
    | secp256r1 -> ECDSA
    |

*/


// https://stackoverflow.com/questions/11493254/can-aes-encryption-and-rsa-digital-signature-scheme-work-together-for-file-encry
// https://github.com/skerkour/black-hat-rust/tree/main/ch_11
// https://cryptobook.nakov.com/digital-signatures


#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NewSeedRequest{
    pub mail: String,
    pub username: String,
    pub phone_number: String,
    pub paypal_id: String,
    pub account_number: String,
    pub device_id: String,
    pub social_id: String,
}


#[derive(Serialize, Deserialize)]
pub struct Data{
    pub amount: u16,
    pub signed_at: i64,
    pub signature: String
}

// https://thalesdocs.com/gphsm/luna/7/docs/network/Content/sdk/using/ecc_curve_cross-reference.htm
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Wallet {
    pub secp256k1_secret_key: Option<String>,
    pub secp256k1_public_key: Option<String>,
    pub secp256k1_public_address: Option<String>,
    pub secp256r1_secret_key: Option<String>,
    pub secp256r1_public_key: Option<String>,
    pub ed25519_secret_key: Option<String>,
    pub ed25519_public_key: Option<String>
}

impl Wallet{

    pub fn generate_keccak256_from(pubk: String) -> String{

        let pubk = PublicKey::from_str(&pubk).unwrap();
        let public_key = pubk.serialize_uncompressed();
        let hash = keccak256(&public_key[1..]);
        let addr: Address = Address::from_slice(&hash[12..]);
        let addr_bytes = addr.as_bytes();
        let addr_string = format!("0x{}", hex::encode(&addr_bytes));
        addr_string

    }

    pub fn new_ed25519() -> Self{

        let rng = ring_rand::SystemRandom::new();
        let pkcs8_bytes = ring_signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keys = ring_signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();

        /* ED25519 keypair */
        let pubkey = keys.public_key().as_ref();
        let prvkey = pkcs8_bytes.as_ref();

        /* converting bytes to hex string */
        let pubkey_string = hex::encode(&pubkey);
        let prvkey_string  = hex::encode(&prvkey);

        Wallet{
            secp256k1_secret_key: None,
            secp256k1_public_key: None,
            secp256k1_public_address: None,
            secp256r1_public_key: None,
            secp256r1_secret_key: None,
            ed25519_public_key: Some(pubkey_string),
            ed25519_secret_key: Some(prvkey_string)
        }

    }
    
    pub fn new_secp256k1(input_seed: NewSeedRequest) -> Self{

        /* generating seed from the input id to create the rng for secp256k1 keypair */
        let input_seed_string = serde_json::to_string(&input_seed).unwrap();
        let input_seed_bytes = input_seed_string.as_bytes();
        let hashed_input_seed = ring::digest::digest(&ring::digest::SHA256, input_seed_bytes);
        let hashed_input_seed_bytes = hashed_input_seed.as_ref();
        
        /* to create the rng we need a 32 bytes seed and we're sure that the hash is 32 bytes cause it's sha256 bits */
        let seed = <[u8; 32]>::try_from(&hashed_input_seed_bytes[0..32]).unwrap(); /* creating a 32 bytes from the first 32 bytes of hashed_input_seed_bytes */
        let rng = &mut StdRng::from_seed(seed);
        
        /* since the secp is going to be built from an specific seed thus the generated keypair will be the same everytime we request a new one */
        let secp = secp256k1::Secp256k1::new();
        let (prvk, pubk) = secp.generate_keypair(rng);
        let prv_str = prvk.display_secret().to_string();

        Wallet{
            secp256k1_secret_key: Some(prv_str), /* (compatible with all evm based chains) */
            secp256k1_public_key: Some(pubk.to_string()),
            secp256k1_public_address: Some(Self::generate_keccak256_from(pubk.to_string())),
            secp256r1_public_key: None,
            secp256r1_secret_key: None,
            ed25519_public_key: None,
            ed25519_secret_key: None
        }
    }

    pub fn new_secp256r1() -> Self{

        /* ECDSA keypairs */
        let ec_key_pair = gen_ec_key_pair(); // generates a pair of Elliptic Curve (ECDSA) keys
        let (private, public) = ec_key_pair.clone().split();
        let hex_pub = Some(hex::encode(public.as_ref()));
        let hex_prv = Some(hex::encode(private.as_ref()));

        Wallet { 
            secp256k1_secret_key: None, 
            secp256k1_public_key: None, 
            secp256k1_public_address: None, 
            secp256r1_secret_key: hex_prv, 
            secp256r1_public_key: hex_pub,
            ed25519_public_key: None,
            ed25519_secret_key: None,
        }

    }

    pub fn ed25519_sign(data: String, prvkey: &str) -> Option<String>{

        /* generating sha25 bits hash of data */
        let hash_data_bytes = Self::generate_sha256_from(data);

        let ed25519 = Self::retrieve_ed25519_keypair(prvkey);

        /* signing the hashed data */
        let signature = ed25519.sign(&hash_data_bytes);
        let sig = signature.as_ref().to_vec();
        Some(hex::encode(&sig))

    }

    pub fn verify_ed25519_signature(sig: String, data: String, pubkey: String) -> bool{

        /* 
            since sig and pubkey are hex string we have to get their bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let sig_bytes = hex::decode(&sig).unwrap();
        let pubkey_bytes = hex::decode(pubkey).unwrap();

        /* generating sha25 bits hash of data */
        let hash_data_bytes = Self::generate_sha256_from(data);

        /* creating the public key  */
        let ring_pubkey = ring_signature::UnparsedPublicKey::new(
            &ring_signature::ED25519, 
            &pubkey_bytes);

        /* 
            Vec<u8> can be coerced to &[u8] slice by taking a reference to it 
            since a pointer to the underlying Vec<u8> means taking a slice of 
            vector with a valid lifetime
        */
        match ring_pubkey.verify(&hash_data_bytes, &sig_bytes){ 
            Ok(_) => true,
            Err(_) => false
        }

    }

    pub fn retrieve_ed25519_keypair(prv_key: &str) -> Ed25519KeyPair{

        /* 
            since prv_key is a hex string we have to get its bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let private_key = hex::decode(prv_key).unwrap();
        let generated_ed25519_keys = Ed25519KeyPair::from_pkcs8(private_key.as_ref()).unwrap();
        generated_ed25519_keys

    }

    pub fn generate_secp256k1_pubkey_from(pk: String) -> Result<PublicKey, secp256k1::Error>{
        let secp256k1_pubkey = PublicKey::from_str(&pk);
        secp256k1_pubkey
    }

    pub fn verify_secp256k1_signature(data: String, sig: Signature, pk: PublicKey) -> Result<(), secp256k1::Error>{

        /* 
            data is required to be passed to the method since we'll compare
            the hash of it with the one inside the signature 
        */
        let data_bytes = data.as_bytes();
        let hashed_data = Message::from_hashed_data::<sha256::Hash>(data_bytes);
            
        /* message is an sha256 bits hashed data */
        let secp = Secp256k1::verification_only();
        secp.verify_ecdsa(&hashed_data, &sig, &pk)

    }

    pub fn retrieve_secp256k1_keypair(secret_key: &str) -> (PublicKey, SecretKey){

        /* 
            since secret_key is a hex string we have to get its bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let prv_bytes = hex::decode(secret_key).unwrap();
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&prv_bytes).unwrap();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        (public_key, secret_key)
    }

    pub fn secp256k1_sign(signer: String, data: String) -> Signature{

        let secret_key = SecretKey::from_str(&signer).unwrap();
        let data_bytes = data.as_bytes();
        let hashed_data = Message::from_hashed_data::<sha256::Hash>(data_bytes);
        
        /* message is an sha256 bits hashed data */
        let secp = Secp256k1::new();

        /* signing the hashed data */
        secp.sign_ecdsa(&hashed_data, &secret_key)

    }

    pub fn retrieve_secp256r1_keypair(pubkey: &str, prvkey: &str) -> themis::keys::KeyPair{

        /* 
            since pubkey and prvkey are hex string we have to get their bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let pubkey_bytes = hex::decode(pubkey).unwrap();
        let prvkey_bytes = hex::decode(prvkey).unwrap();

        /* building ECDSA keypair from pubkey and prvkey slices */
        let ec_pubkey = EcdsaPublicKey::try_from_slice(&pubkey_bytes).unwrap();
        let ec_prvkey = EcdsaPrivateKey::try_from_slice(&prvkey_bytes).unwrap();
        let generated_ec_keypair = ThemisKeyPair::try_join(ec_prvkey, ec_pubkey).unwrap();
        generated_ec_keypair

    }

    pub fn secp256r1_sign(signer: String, data: String) -> Option<String>{

        /* 
            since signer is a hex string we have to get its bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let prvkey_bytes = hex::decode(signer).unwrap();
        let ec_prvkey = EcdsaPrivateKey::try_from_slice(&prvkey_bytes).unwrap();
        let ec_signer = SecureSign::new(ec_prvkey.clone());

        /* generating sha25 bits hash of data */
        let hash_data_bytes = Self::generate_sha256_from(data);
    
        /* generating signature from the hashed data */
        let ec_sig = ec_signer.sign(&hash_data_bytes).unwrap();
        
        /* converting the signature bytes into hex string */
        Some(hex::encode(&ec_sig))

    }

    pub fn verify_secp256r1_signature(signature: &str, pubkey: &str) -> Result<Vec<u8>, themis::Error>{

        /* 
            since signature and pubkey are hex string we have to get their bytes using 
            hex::decode() cause calling .as_bytes() on the hex string converts
            the hex string itself into bytes and it doesn't return the acutal bytes
        */
        let signature_bytes = hex::decode(signature).unwrap();
        let pubkey_bytes = hex::decode(pubkey).unwrap();

        /* building the public key from public key bytes */
        let Ok(ec_pubkey) = EcdsaPublicKey::try_from_slice(&pubkey_bytes) else{
            let err = EcdsaPublicKey::try_from_slice(&pubkey_bytes).unwrap_err();
            return Err(err); /* can't build pubkey from the passed in slice */
        };

        /* building the verifier from the public key */
        let ec_verifier = SecureVerify::new(ec_pubkey.clone());

        /* verifying the signature byte which returns the hash of data in form of vector of utf8 bytes */
        let encoded_data = ec_verifier.verify(&signature_bytes);

        /* this is the encoded sha256 bits hash of data */
        encoded_data

    }

    pub fn generate_sha256_from(data: String) -> [u8; 32]{

        /* generating sha25 bits hash of data */
        let data_bytes = data.as_bytes();
        let hash_data = sha256::Hash::hash(data_bytes);
        let hash_data_bytes = hash_data.as_byte_array();
        hash_data_bytes.to_owned()

    }
    
}

pub struct Contract{
    pub wallet: Wallet,
    pub iat: i64,
    pub owner: &'static str,
    pub data: Option<Data>,
}

impl Contract{

    fn new_with_ed25519(owner: &str) -> Self{
        
        let static_owner = string_to_static_str(owner.to_string());
        let wallet = Wallet::new_ed25519();

        Self { 
            wallet,
            iat: chrono::Local::now().timestamp_nanos(), 
            owner: static_owner,
            data: None
        }
        
    }

    fn new_with_secp256r1(owner: &str) -> Self{
        
        let static_owner = string_to_static_str(owner.to_string());
        let wallet = Wallet::new_secp256r1();

        Self { 
            wallet,
            iat: chrono::Local::now().timestamp_nanos(), 
            owner: static_owner,
            data: None
        }
        
    }

    fn new_with_secp256k1(owner: &str) -> Self{
        
        let static_owner = string_to_static_str(owner.to_string());
        let wallet = Wallet::new_secp256k1(NewSeedRequest::default());

        Self { 
            wallet,
            iat: chrono::Local::now().timestamp_nanos(), 
            owner: static_owner,
            data: None
        }
        
    }

}



/* ----------------------------- */
//              TESTS   
/* ----------------------------- */

#[cfg(test)]
pub mod tests{

    use super::*;

    #[test]
    pub fn ed25519_test() -> Result<(), ()>{
        
        let mut data = Data{
            amount: 10, 
            signature: "".to_string(),
            signed_at: 0,
        };
        let stringify_data = serde_json::to_string_pretty(&data).unwrap();

        /* wallet operations */

        let contract = Contract::new_with_ed25519("0xDE6D7045Df57346Ec6A70DfE1518Ae7Fe61113f4");
        
        let signature_hex = Wallet::ed25519_sign(stringify_data.clone(), contract.wallet.ed25519_secret_key.as_ref().unwrap());
        
        let is_verified = Wallet::verify_ed25519_signature(signature_hex.clone().unwrap(), stringify_data, contract.wallet.ed25519_public_key.unwrap());

        let keypair = Wallet::retrieve_ed25519_keypair(
            /* 
                unwrap() takes the ownership of the type hence we must borrow 
                the type before calling it using as_ref() 
            */
            contract.wallet.ed25519_secret_key.unwrap().as_str()
        );

        data.signature = signature_hex.unwrap();
        data.signed_at = chrono::Local::now().timestamp_nanos();

        match is_verified{
            true => Ok(()),
            false => Err(())
        }

    }

    #[test]
    pub fn secp256r1_test() -> Result<(), themis::Error>{

        let mut data = Data{
            amount: 10, 
            signature: "".to_string(),
            signed_at: 0,
        };
        let stringify_data = serde_json::to_string_pretty(&data).unwrap();

        /* wallet operations */
        
        let contract = Contract::new_with_secp256r1("0xDE6D7045Df57346Ec6A70DfE1518Ae7Fe61113f4");

        let hashed_data = Wallet::generate_sha256_from(stringify_data.clone());

        let signature_hex = Wallet::secp256r1_sign(contract.wallet.secp256r1_secret_key.as_ref().unwrap().to_string(), stringify_data.clone());

        let verification_result = Wallet::verify_secp256r1_signature(&signature_hex.clone().unwrap(), contract.wallet.secp256r1_public_key.as_ref().unwrap().as_str());

        let keypair = Wallet::retrieve_secp256r1_keypair(
            /* 
                unwrap() takes the ownership of the type hence we must borrow 
                the type before calling it using as_ref() 
            */
            contract.wallet.secp256r1_public_key.as_ref().unwrap(),
            contract.wallet.secp256r1_secret_key.as_ref().unwrap() 
        );

        data.signature = signature_hex.unwrap();
        data.signed_at = chrono::Local::now().timestamp_nanos();

        match verification_result{
            Ok(hashed_data_vector) => {

                println!("hashed data inside sig: [{:?}]", &hashed_data_vector[..23]);
                println!("hashed data: [{:?}]", hashed_data);

                Ok(())
            },
            Err(e) => Err(e)
        }

    }

    #[test]
    pub fn secp256k1_test() -> Result<(), secp256k1::Error>{

        let mut data = Data{
            amount: 10, 
            signature: "".to_string(),
            signed_at: 0,
        };
        let stringify_data = serde_json::to_string_pretty(&data).unwrap();

        /* wallet operations */

        let contract = Contract::new_with_secp256k1("0xDE6D7045Df57346Ec6A70DfE1518Ae7Fe61113f4");

        let signature = Wallet::secp256k1_sign(contract.wallet.secp256k1_secret_key.as_ref().unwrap().to_string(), stringify_data.clone());

        let pubkey = Wallet::generate_secp256k1_pubkey_from(contract.wallet.secp256k1_public_key.as_ref().unwrap().to_string());

        let keypair = Wallet::retrieve_secp256k1_keypair(
            /* 
                unwrap() takes the ownership of the type hence we must borrow 
                the type before calling it using as_ref() 
            */
            contract.wallet.secp256k1_secret_key.as_ref().unwrap().as_str()
        );

        data.signature = signature.to_string();
        data.signed_at = chrono::Local::now().timestamp_nanos();

        match pubkey{
            Ok(pk) => {

                let verification_result = Wallet::verify_secp256k1_signature(stringify_data, signature, pk);
                match verification_result{
                    Ok(_) => Ok(()),
                    Err(e) => Err(e) 
                }

            },
            Err(e) => Err(e)
        }


    }
 

}

