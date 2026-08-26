import secrets

from fastapi import HTTPException, Request, status


def require_bearer(request: Request) -> None:
    expected = request.app.state.settings.api_token.get_secret_value()
    scheme, _, supplied = request.headers.get('authorization', '').partition(' ')
    if scheme.lower() != 'bearer' or not supplied or not secrets.compare_digest(supplied, expected):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail='invalid bearer token',
            headers={'WWW-Authenticate': 'Bearer'},
        )
