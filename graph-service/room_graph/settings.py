from pydantic import Field, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix='GRAPH_', case_sensitive=False)

    api_token: SecretStr = Field(min_length=16)
    falkordb_host: str = '127.0.0.1'
    falkordb_port: int = Field(default=6379, ge=1, le=65535)
    falkordb_username: str | None = None
    falkordb_password: SecretStr | None = None
    llm_api_key: SecretStr = Field(min_length=1)
    llm_base_url: str | None = None
    llm_model: str = 'gpt-4.1-mini'
    embedding_api_key: SecretStr | None = None
    embedding_base_url: str | None = None
    embedding_model: str = 'text-embedding-3-small'
    embedding_dimensions: int = Field(default=1536, ge=1, le=65_536)
    max_concurrent_writes: int = Field(default=4, ge=1, le=32)
    snapshot_limit: int = Field(default=250, ge=1, le=1_000)

    @property
    def resolved_embedding_key(self) -> str:
        key = self.embedding_api_key or self.llm_api_key
        return key.get_secret_value()
