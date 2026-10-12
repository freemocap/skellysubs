"""Exercise account check-in and installation administration through HTTP."""
from uuid import UUID, uuid4

import httpx
import pytest

from server.app import main
from server.app.accounting import quota
from server.app.identity import auth
from server.tests.diagnostics.test_admin import database, cookie


@pytest.mark.asyncio
async def test_full_installation_history_allows_account_check_and_admin_cleanup(database):
    target = 'google:test01'
    devices = database.collection('users').document(target).collection(quota.DEVICES)
    for number in range(1, 101):
        quota.record_device(database, target, install_id=str(UUID(int=number)), platform='android', app_version='test')
    token = auth.issue_session_token(user_id=target, signing_key=main.CFG.jwt_signing_key)
    async with httpx.AsyncClient(transport=httpx.ASGITransport(app=main.app), base_url='https://test.invalid') as client:
        response = await client.get('/v1/me', headers={'authorization': f'Bearer {token}', 'x-skellyspeak-install': str(UUID(int=101))})
        assert response.status_code == 200, response.text
        assert len(list(devices.stream())) == 100
        command = {'action': 'clear_installations', 'target': target, 'operation_id': str(uuid4()), 'expected_revision': 0, 'values': {}}
        assert (await client.post('/admin/api/change', json=command)).status_code == 401
        client.headers['cookie'] = cookie(database)
        detail = (await client.get(f'/admin/api/users/{target}?days=7')).json()
        assert detail['installation_policy'] == {'maximum_retained': 100, 'inactivity_days': 90}
        assert len(detail['devices']) == 100
        assert detail['devices'][0]['id'] == str(UUID(int=101))
        assert (await client.post('/admin/api/change', json=command)).status_code == 403
        headers = {'origin': main.CFG.public_base_url, 'x-admin-action': '1'}
        assert (await client.post('/admin/api/change', json=command, headers=headers)).status_code == 200
        assert list(devices.stream()) == []
        assert (await client.post('/admin/api/change', json=command, headers=headers)).json()['replayed']
        response = await client.get('/v1/me', headers={'authorization': f'Bearer {token}', 'x-skellyspeak-install': str(UUID(int=101))})
        assert response.status_code == 200
        assert len(list(devices.stream())) == 1
