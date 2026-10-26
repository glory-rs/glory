#!/usr/bin/env python3
"""Derive signing entitlements from an explicit, unexpired application profile."""
import datetime
import pathlib
import plistlib
import sys

profile = plistlib.loads(pathlib.Path(sys.argv[1]).read_bytes())
expiry = profile['ExpirationDate'].replace(tzinfo=datetime.timezone.utc)
if expiry <= datetime.datetime.now(datetime.timezone.utc):
    raise SystemExit('Provisioning profile has expired')
entitlements = dict(profile['Entitlements'])
app = entitlements['application-identifier']
team = profile['ApplicationIdentifierPrefix'][0]
identifier = f'{team}.{sys.argv[2]}'
if app != identifier and not (app.endswith('.*') and identifier.startswith(app[:-1])):
    raise SystemExit('Provisioning profile does not authorize this bundle identifier')
entitlements['application-identifier'] = identifier
# A wildcard keychain grant must be specialized to this app before codesign.
entitlements['keychain-access-groups'] = [
    identifier if value == f'{team}.*' else value
    for value in entitlements.get('keychain-access-groups', [])
]
pathlib.Path(sys.argv[3]).write_bytes(plistlib.dumps(entitlements))
