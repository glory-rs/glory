"""Publication retry tests; no network, credentials or registry mutations."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tarfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

spec = importlib.util.spec_from_file_location('publisher', Path(__file__).with_name('release.py'))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class PublicationRetryTests(unittest.TestCase):
    def test_success_does_not_contact_registry(self):
        with patch.object(publisher.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout='')), \
             patch.object(publisher.urllib.request, 'urlopen') as network:
            publisher.publish(['cargo', 'publish'])
            network.assert_not_called()

    def test_compiler_or_auth_failure_is_not_retried(self):
        for diagnostic in ('compiler error', '403 Forbidden', '429 without a retry time'):
            with self.subTest(diagnostic=diagnostic), \
                 patch.object(publisher.subprocess, 'run', return_value=SimpleNamespace(returncode=101, stdout=diagnostic)) as command, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(subprocess.CalledProcessError):
                    publisher.publish(['cargo', 'publish'])
                self.assertEqual(command.call_count, 1)

    def test_only_explicit_rate_limit_waits_until_registry_time(self):
        limited = SimpleNamespace(returncode=101, stdout='429 Too Many Requests: try again after Thu, 29 Oct 2026 07:50:05 GMT')
        response = MagicMock()
        response.__enter__.return_value.headers = {'Date': 'Thu, 29 Oct 2026 07:50:00 GMT'}
        with patch.object(publisher.subprocess, 'run', side_effect=[limited, SimpleNamespace(returncode=0, stdout='')]) as command, \
             patch.object(publisher.urllib.request, 'urlopen', return_value=response), \
             patch.object(publisher.time, 'monotonic', side_effect=[0, 1, 2, 11]), \
             patch.object(publisher.time, 'sleep') as sleep, \
             contextlib.redirect_stdout(io.StringIO()):
            publisher.publish(['cargo', 'publish'])
            self.assertEqual(command.call_count, 2)
            sleep.assert_called_once_with(8)


class ReleasePlanTests(unittest.TestCase):
    def validate(self, mutate=lambda plan, packages: None, tag=None):
        parent=Path(os.environ.get('CARGO_TARGET_DIR','D:/BuildCache/release-tests/target')).parent
        parent.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='publisher-test-',dir=parent) as directory:
            root=Path(directory)
            target=root/'target'
            (root/'Cargo.toml').write_text('[workspace.package]\nversion="0.6.0"\n[package]\nversion="0.0.1"\n',encoding='utf-8')
            plan={'repository':'example/owned','allowed_prefixes':['owned'],'tag_prefix':'v','uniform_version':True,
                  'packages':['owned-base','owned-app']}
            packages=[{'name':name,'version':'0.6.0','repository':'https://github.com/example/owned','dependencies':[],
                       'manifest_path':str(root/'Cargo.toml')} for name in plan['packages']]
            mutate(plan,packages)
            (root/'release.json').write_text(json.dumps(plan),encoding='utf-8')
            metadata=json.dumps({'target_directory':str(target),'packages':packages})
            arguments=['release.py']+(['--tag',tag] if tag else [])
            with patch.object(publisher,'ROOT',root),patch.object(publisher,'run',return_value=metadata), \
                 patch.object(publisher.sys,'argv',arguments),patch.dict(os.environ,{'CARGO_TARGET_DIR':str(target),'GITHUB_REF_TYPE':'branch','GITHUB_REF_NAME':'main'}), \
                 contextlib.redirect_stdout(io.StringIO()) as output:
                publisher.main()
                return output.getvalue()

    def test_workspace_version_precedes_private_example_version(self):
        self.assertIn('"tag": "v0.6.0"',self.validate())

    def test_wrong_tag_is_rejected(self):
        with self.assertRaises(SystemExit):
            self.validate(tag='v0.0.1')

    def test_wrong_repository_namespace_version_and_disabled_package_are_rejected(self):
        mutations=[lambda p,v:v[0].update(repository='https://github.com/another/owner'),
                   lambda p,v:p.update(allowed_prefixes=['other']),
                   lambda p,v:v[0].update(version='0.3.1'),
                   lambda p,v:v[0].update(publish=[])]
        for mutation in mutations:
            with self.subTest(mutation=mutation),self.assertRaises(SystemExit):
                self.validate(mutation)

    def test_git_external_path_and_reversed_order_are_rejected(self):
        dependency={'name':'foreign','req':'*','kind':None,'source':None,'path':'D:/unpublished'}
        def git(plan,packages):
            packages[1]['dependencies']=[dict(dependency,source='git+https://github.com/example/foreign')]
        def path(plan,packages):
            packages[1]['dependencies']=[dependency]
        def order(plan,packages):
            plan['packages'].reverse()
            packages[1]['dependencies']=[dict(dependency,name='owned-base',req='=0.6.0')]
        for mutation in (git,path,order):
            with self.subTest(mutation=mutation),self.assertRaises(SystemExit):
                self.validate(mutation)

    def test_reviewed_exact_external_path_is_allowed(self):
        def mutation(plan,packages):
            plan['external_path_dependencies']={'foreign':'=0.6.0'}
            packages[1]['dependencies']=[{'name':'foreign','req':'=0.6.0','kind':None,'source':None,'path':'D:/published'}]
        self.assertIn('v0.6.0',self.validate(mutation))


class ArchiveTests(unittest.TestCase):
    def inspect(self, license_content, sha='a'*40, dirty=False):
        parent=Path(os.environ.get('CARGO_TARGET_DIR','D:/BuildCache/release-tests/target')).parent
        parent.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='archive-test-',dir=parent) as directory:
            archive=Path(directory)/'test.crate'
            with tarfile.open(archive,'w:gz') as output:
                for name,data in [('pkg/.cargo_vcs_info.json',json.dumps({'git':{'sha1':sha,'dirty':dirty}}).encode()),
                                  ('pkg/LICENSE',license_content)]:
                    entry=tarfile.TarInfo(name);entry.size=len(data)
                    output.addfile(entry,io.BytesIO(data))
            return publisher.validate_archive(archive,'a'*40)

    def test_full_license_is_accepted(self):
        self.assertEqual(self.inspect(b'Apache License\nVersion 2.0\n')['git']['sha1'],'a'*40)

    def test_windows_link_stub_is_rejected_before_upload(self):
        with self.assertRaisesRegex(SystemExit,'unresolved link'):
            self.inspect(b'../LICENSE')

    def test_wrong_or_dirty_source_is_rejected(self):
        for arguments in ({'sha':'b'*40},{'dirty':True}):
            with self.subTest(arguments=arguments),self.assertRaises(SystemExit):
                self.inspect(b'Apache License',**arguments)


if __name__ == '__main__':
    unittest.main()
