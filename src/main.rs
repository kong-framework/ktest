use kong::{json, kroute, server, ErrorResponse, Kong, Kontrol, Method};
use kong_kontrollers::accounts::{
    create::CreateAccountKontroller, database::Database as AccountsDB,
};
use kong_kontrollers::blog::{create::CreateBlogPostKontroller, database::Database as BlogsDB};
use kong_kontrollers::login::{is_admin, LoginKontroller};
use kong_kontrollers::newsletter::{
    database::Database as NewsletterDB, subscribe::SubscribeNewsletterKontroller,
};
use std::sync::{Arc, Mutex};

const TEST_ACCOUNTS_DB: &str = "TEST_ACCOUNTS_DATABASE.sqlite";
const TEST_BLOGS_DB: &str = "TEST_BLOGS_DATABASE.sqlite";
const TEST_NEWSLETTER_DB: &str = "TEST_NEWSLETTER_DATABASE.sqlite";

fn main() {
    let accounts_database = Arc::new(Mutex::new(AccountsDB::new(TEST_ACCOUNTS_DB)));
    accounts_database.lock().unwrap().connect().unwrap();

    let blogs_database = Arc::new(Mutex::new(BlogsDB::new(TEST_BLOGS_DB)));
    blogs_database.lock().unwrap().connect().unwrap();

    let newsletter_database = Arc::new(Mutex::new(NewsletterDB::new(TEST_NEWSLETTER_DB)));
    newsletter_database.lock().unwrap().connect().unwrap();

    kroute(vec![
        Box::new(CreateAccountKontroller {
            address: "/accounts".to_string(),
            method: Method::Post,
            database: accounts_database.clone(),
        }),
        Box::new(LoginKontroller {
            address: "/login".to_string(),
            method: Method::Post,
            database: accounts_database.clone(),
        }),
        Box::new(CreateBlogPostKontroller {
            address: "/blog".to_string(),
            method: Method::Post,
            database: blogs_database.clone(),
            accounts_database: accounts_database.clone(),
        }),
        Box::new(PrivateKontroller {
            address: "/private".to_string(),
            method: Method::Get,
            database: accounts_database.clone(),
        }),
        Box::new(SubscribeNewsletterKontroller {
            address: "/newsletter".to_string(),
            method: Method::Post,
            database: newsletter_database.clone(),
        }),
    ]);
}

struct PrivateKontroller {
    /// Endpoint address
    address: String,
    /// Endpoint HTTP method
    method: Method,
    /// Accounts database
    database: Arc<Mutex<AccountsDB>>,
}
impl Kontrol for PrivateKontroller {
    fn address(&self) -> String {
        self.address.clone()
    }

    fn method(&self) -> Method {
        self.method
    }

    fn kontrol(&self, kong: &Kong) -> server::Response {
        if let Some(k) = &kong.kpassport {
            if let Ok(admin) = is_admin(k, self.database.clone()) {
                if admin {
                    let res = json!({ "message": "Hello World" });
                    server::Response::json(&res).with_status_code(200)
                } else {
                    ErrorResponse::unauthorized()
                }
            } else {
                ErrorResponse::internal()
            }
        } else {
            ErrorResponse::unauthorized()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use kong_kontrollers::accounts::inputs::AccountCreationInput;
    use kong_kontrollers::login::inputs::AccountLoginInput;
    use reqwest::{blocking::multipart, StatusCode};
    const ADDRESS: &str = "http://localhost:7878";

    #[test]
    fn test_register_account_login() {
        remove_test_dbs();

        let private_route = format!("{ADDRESS}/private");
        let register_route = format!("{ADDRESS}/accounts");
        let login_route = format!("{ADDRESS}/login");
        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();

        // try to access a private route without being logged in
        let res = client.get(&private_route).send().unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // register admin account
        let account = AccountCreationInput {
            username: "admin".to_string(),
            email: Some("admin@example.com".to_string()),
            password: "1234567890".to_string(),
        };
        let res = client.post(&register_route).json(&account).send().unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);

        // try to register account with already existing credentials
        let res = client.post(register_route).json(&account).send().unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // try to login with wrong credentials
        let login_info = AccountLoginInput {
            username: "admin".to_string(),
            password: "wrong_password".to_string(),
        };
        let res = client.post(&login_route).json(&login_info).send().unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // try to login with correct credentials
        let login_info = AccountLoginInput {
            username: "admin".to_string(),
            password: "1234567890".to_string(),
        };
        let res = client.post(login_route).json(&login_info).send().unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // try to access a private route
        let res = client.get(&private_route).send().unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    fn remove_test_dbs() {
        let test_db_path = std::path::Path::new(TEST_ACCOUNTS_DB);
        if std::path::Path::exists(test_db_path) {
            std::fs::remove_file(test_db_path).unwrap();
        }

        let test_db_path = std::path::Path::new(TEST_BLOGS_DB);
        if std::path::Path::exists(test_db_path) {
            std::fs::remove_file(test_db_path).unwrap();
        }
    }

    #[test]
    fn test_create_blog_post() {
        remove_test_dbs();

        let register_route = format!("{ADDRESS}/accounts");
        let login_route = format!("{ADDRESS}/login");
        let url = format!("{ADDRESS}/blog");
        let form = multipart::Form::new()
            .text("title", "Test title")
            .text("subtitle", "Test subtitle")
            .file("cover", "./test.png")
            .unwrap()
            .text("content", "Test Content");

        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();
        let res = client.post(&url).multipart(form).send().unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // create admin new account
        let account = AccountCreationInput {
            username: "admin".to_string(),
            email: Some("admin@example.com".to_string()),
            password: "1234567890".to_string(),
        };
        client.post(register_route).json(&account).send().unwrap();

        // login
        let login_info = AccountLoginInput {
            username: "admin".to_string(),
            password: "1234567890".to_string(),
        };
        client.post(login_route).json(&login_info).send().unwrap();

        // Post blog
        let form = multipart::Form::new()
            .text("title", "Test title")
            .text("subtitle", "Test subtitle")
            .file("cover", "./test.png")
            .unwrap()
            .text("content", "Test Content");

        let res = client.post(&url).multipart(form).send().unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[test]
    fn test_subscribe_newsletter() {
        remove_test_dbs();

        let url = format!("{ADDRESS}/newsletter");
        let form = multipart::Form::new().text("email", "test@example.com");

        let client = reqwest::blocking::Client::builder().build().unwrap();
        let res = client.post(&url).multipart(form).send().unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
}
