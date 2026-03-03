"""Application configuration loaded from environment."""

import os
import json
from pathlib import Path

BASE_DIR = Path(__file__).parent

DATABASE_URL = os.getenv("DATABASE_URL", "sqlite:///db.sqlite3")
SECRET_KEY = os.getenv("SECRET_KEY", "dev-secret")
DEBUG = json.loads(os.getenv("DEBUG", "true"))
