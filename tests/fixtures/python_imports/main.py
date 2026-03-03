"""Application entry point."""

import os
import sys
import logging

from fastapi import FastAPI
from app.routes import router
from app.services import UserService

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="Python Imports Test")
app.include_router(router)

service = UserService()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=int(os.getenv("PORT", "8000")))
