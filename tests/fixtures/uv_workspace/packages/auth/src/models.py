"""
Auth domain models.
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Optional


@dataclass
class UserProfile:
    """Authenticated user profile returned from token validation."""

    id: str
    email: str
    name: str
    role: str = "customer"
    avatar_url: Optional[str] = None
    created_at: Optional[datetime] = None


@dataclass
class TokenPayload:
    """JWT token payload structure."""

    user_id: str
    email: str
    role: str
    issued_at: datetime = field(default_factory=datetime.utcnow)
    expires_at: Optional[datetime] = None


@dataclass
class AuthConfig:
    """Authentication configuration."""

    jwt_secret: str = ""
    jwt_algorithm: str = "HS256"
    token_expiry_minutes: int = 60
    refresh_token_expiry_days: int = 30
