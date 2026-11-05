import tempfile
from pathlib import Path
import subprocess
import unittest
from unittest.mock import Mock

from nvibrant.restore import restore
from nvibrant.settings import SettingsStore

PORT = dict(index=5, display=0, connector=5, type='DisplayPort', connected=True)


class RestoreTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.store = SettingsStore(Path(temporary.name) / 'settings.json')
        self.backend = Mock()
        self.backend.discover.return_value = [dict(PORT, index=2),
                                             dict(PORT, index=0, connector=0)]
        self.factory = Mock(return_value=self.backend)
        self.sleep = Mock()

    def run_restore(self):
        restore(self.store, self.factory, attempts=3, sleep=self.sleep)

    def test_restore_from_disk_matches_connector_and_does_not_rewrite_settings(self):
        self.store.save(1, [PORT], {5: 0})
        original = self.store.path.read_bytes()
        self.store = SettingsStore(self.store.path)
        self.run_restore()
        self.factory.assert_called_once_with(gpu=1)
        self.backend.apply.assert_called_once_with(self.backend.discover.return_value,
                                                  {2: 0}, preserve_unspecified=True)
        self.assertEqual(self.store.path.read_bytes(), original)
        self.sleep.assert_not_called()

    def test_empty_or_corrupt_preferences_never_access_hardware(self):
        self.run_restore()
        self.factory.assert_not_called()
        self.store.path.write_text('{broken')
        with self.assertRaises(ValueError):
            self.run_restore()
        self.factory.assert_not_called()

    def test_waits_for_outputs_and_retries_driver_timeout(self):
        self.store.save(0, [PORT], {5: 512})
        self.backend.discover.side_effect = [[], [PORT], [PORT]]
        self.backend.apply.side_effect = [subprocess.TimeoutExpired('backend', 15), 'Success']
        self.run_restore()
        self.assertEqual(self.backend.discover.call_count, 3)
        self.assertEqual(self.sleep.call_count, 2)

    def test_retries_are_bounded_and_report_failure(self):
        self.store.save(0, [PORT], {5: 512})
        self.backend.discover.return_value = [dict(PORT, connected=False)]
        with self.assertRaisesRegex(RuntimeError, 'GPU 0: No connected outputs'):
            self.run_restore()
        self.backend.apply.assert_not_called()
        self.assertEqual(self.backend.discover.call_count, 3)

    def test_successful_gpu_is_not_reapplied_while_another_gpu_retries(self):
        self.store.save(0, [PORT], {5: 512})
        self.store.save(1, [PORT], {5: 768})
        second = Mock()
        second.discover.side_effect = [RuntimeError('not ready'), [PORT]]
        self.factory.side_effect = lambda gpu: self.backend if gpu == 0 else second
        self.run_restore()
        self.backend.apply.assert_called_once()
        second.apply.assert_called_once_with([PORT], {5: 768}, preserve_unspecified=True)
