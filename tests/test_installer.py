"""Exercise installer boundaries without package-manager or GPU writes."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('nvibrant_install', ROOT / 'scripts/install.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='nvibrant-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.prefix = self.root / 'prefix with spaces'
        self.build = self.root / 'build'
        self.build.mkdir()
        # A deliberate fake backend lets us verify argument forwarding and location.
        self.backend = self.build / 'nvibrant'
        self.backend.write_text('#!/bin/sh\nprintf "backend:%s\\n" "$1"\n')
        self.backend.chmod(0o755)

    def install(self):
        installer.install(ROOT, self.build, self.prefix, '615.78.08')

    def test_installed_launchers_survive_source_build_removal(self):
        self.install()
        self.backend.unlink()
        gui = self.prefix / 'bin/nvibrant-gtk'
        cli = self.prefix / 'bin/nvibrant'
        result = subprocess.run([gui, '--help'], cwd='/', text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('--demo', result.stdout)
        self.assertEqual(subprocess.check_output([cli, '--list-json'], text=True).strip(),
                         'backend:--list-json')
        desktop = (self.prefix / 'share/applications' / installer.DESKTOP).read_text()
        self.assertIn(f'Exec="{gui}"', desktop)
        self.assertIn(f'Icon={installer.APP_ID}', desktop)
        icon = self.prefix / 'share/icons/hicolor/scalable/apps' / f'{installer.APP_ID}.svg'
        self.assertEqual(icon.read_bytes(), (ROOT / 'nvibrant/assets' / icon.name).read_bytes())
        self.assertFalse((self.prefix / 'lib/nvibrant/nvibrant/main.cpp').exists())

    def test_reinstall_and_uninstall_leave_other_prefix_files_alone(self):
        self.prefix.mkdir()
        unrelated = self.prefix / 'keep-me'
        unrelated.write_text('untouched')
        self.install()
        self.install()
        installer.uninstall(self.prefix)
        self.assertEqual(unrelated.read_text(), 'untouched')
        for path in installer.locations(self.prefix):
            self.assertFalse(installer.exists(path), str(path))

    def test_failed_copy_preserves_previous_installation(self):
        self.install()
        old_backend = (self.prefix / 'lib/nvibrant/build/nvibrant').read_bytes()
        with patch.object(installer.shutil, 'copytree', side_effect=OSError('disk full')):
            with self.assertRaisesRegex(OSError, 'disk full'):
                self.install()
        self.assertEqual((self.prefix / 'lib/nvibrant/build/nvibrant').read_bytes(), old_backend)
        self.assertFalse(list((self.prefix / 'lib').glob('.nvibrant-install-*')))

    def test_refuses_to_overwrite_existing_unrelated_cli(self):
        path = self.prefix / 'bin/nvibrant'
        path.parent.mkdir(parents=True)
        path.write_text('unrelated')
        with self.assertRaisesRegex(RuntimeError, 'overwritten'):
            self.install()
        self.assertEqual(path.read_text(), 'unrelated')

    def test_bad_arguments_fail_before_build(self):
        result = subprocess.run(['bash', ROOT / 'build.sh', 'build', '--jobs', '0'],
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('positive integer', result.stderr)

    def test_unsupported_architecture_fails_before_build(self):
        tools = self.root / 'tools'
        tools.mkdir()
        uname = tools / 'uname'
        uname.write_text('#!/bin/sh\ncase "$1" in -s) echo Linux;; *) echo riscv64;; esac\n')
        uname.chmod(0o755)
        env = dict(os.environ, PATH=f'{tools}:{os.environ["PATH"]}')
        result = subprocess.run(['bash', ROOT / 'build.sh', 'check'], env=env,
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Unsupported architecture', result.stderr)

    def test_stdin_script_does_not_need_a_local_script_filename(self):
        result = subprocess.run(['bash', '-s', '--', 'check'], input=(ROOT / 'build.sh').read_text(),
                                cwd=self.root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('dependencies are ready', result.stdout)


if __name__ == '__main__':
    unittest.main()
