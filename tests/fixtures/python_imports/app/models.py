"""Data models for the application."""

import dataclasses
from typing import Optional, List
from datetime import datetime


@dataclasses.dataclass
class User:
    id: int
    name: str
    email: str
    created_at: datetime
    is_active: bool = True
    phone: Optional[str] = None


@dataclasses.dataclass
class Product:
    id: int
    title: str
    price: float
    tags: List[str] = dataclasses.field(default_factory=list)
