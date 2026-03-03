"""Business logic services."""

import hashlib
import logging
from collections import defaultdict

from .models import User, Product

logger = logging.getLogger(__name__)


class UserService:
    def __init__(self):
        self._cache = defaultdict(list)

    def hash_password(self, password: str) -> str:
        return hashlib.sha256(password.encode()).hexdigest()

    def find_active_users(self, users: list) -> list:
        return [u for u in users if u.is_active]


class ProductService:
    def search(self, query: str, products: list) -> list:
        return [p for p in products if query.lower() in p.title.lower()]
