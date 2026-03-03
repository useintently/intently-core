/// Core domain types for the monorepo.
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

pub struct UserRepository {
    users: Vec<User>,
}

impl UserRepository {
    pub fn new() -> Self {
        Self { users: Vec::new() }
    }

    pub fn find_by_id(&self, id: u64) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }

    pub fn create(&mut self, user: User) -> &User {
        self.users.push(user);
        self.users.last().unwrap()
    }
}

pub trait AuthService {
    fn verify_token(&self, token: &str) -> bool;
}
