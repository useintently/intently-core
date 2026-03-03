use mono_core::{User, UserRepository, AuthService};

pub struct ApiHandler {
    repo: UserRepository,
}

impl ApiHandler {
    pub fn new() -> Self {
        Self {
            repo: UserRepository::new(),
        }
    }

    pub fn get_user(&self, id: u64) -> Option<&User> {
        self.repo.find_by_id(id)
    }

    pub fn create_user(&mut self, name: String, email: String) -> &User {
        let user = User {
            id: 1,
            name,
            email,
        };
        self.repo.create(user)
    }
}

pub struct JwtAuth;

impl AuthService for JwtAuth {
    fn verify_token(&self, token: &str) -> bool {
        !token.is_empty()
    }
}
