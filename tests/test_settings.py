import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from nvibrant.settings import SettingsStore, config_path

PORT = dict(index=5, display=0, connector=5, type='DisplayPort', connected=True)


class SettingsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='nvibrant-settings-')
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / 'nvibrant/settings.json'
        self.store = SettingsStore(self.path)

    def test_missing_settings_do_not_create_files(self):
        self.store.load()
        self.assertEqual(self.store.last_gpu, 0)
        self.assertIsNone(self.store.value(0, PORT))
        self.assertFalse(self.path.parent.exists())

    def test_successful_values_round_trip_and_natural_is_remembered(self):
        self.store.save(1, [PORT], {5: 512})
        fresh = SettingsStore(self.path)
        fresh.load()
        self.assertEqual(fresh.last_gpu, 1)
        self.assertEqual(fresh.value(1, PORT), 512)
        fresh.save(1, [PORT], {5: 0})
        self.store.load()
        self.assertEqual(self.store.value(1, PORT), 0)

    def test_other_gpu_and_disconnected_values_survive_a_new_window_save(self):
        self.store.save(0, [PORT], {5: 256})
        stale_window = SettingsStore(self.path)
        stale_window.load()
        self.store.save(1, [PORT], {5: 768})
        stale_window.save(0, [dict(PORT, connected=False)], {})
        latest = SettingsStore(self.path)
        latest.load()
        self.assertEqual(latest.value(0, PORT), 256)
        self.assertEqual(latest.value(1, PORT), 768)

    def test_output_identity_does_not_depend_on_flat_argument_index(self):
        self.store.save(0, [PORT], {5: 512})
        self.assertEqual(self.store.value(0, dict(PORT, index=2)), 512)
        self.assertIsNone(self.store.value(0, dict(PORT, connector=2)))
        self.assertIsNone(self.store.value(1, PORT))

    def test_failed_replace_keeps_previous_file_and_cleans_temporary_file(self):
        self.store.save(0, [PORT], {5: 256})
        old = self.path.read_bytes()
        with patch('nvibrant.settings.os.replace', side_effect=OSError('disk full')):
            with self.assertRaisesRegex(OSError, 'disk full'):
                self.store.save(0, [PORT], {5: 768})
        self.assertEqual(self.path.read_bytes(), old)
        self.assertEqual(self.store.value(0, PORT), 256)
        self.assertFalse(list(self.path.parent.glob('*.tmp')))

    def test_invalid_file_is_reported_without_overwriting_it(self):
        self.path.parent.mkdir()
        self.path.write_text('{broken')
        with self.assertRaisesRegex(ValueError, 'Cannot read saved settings'):
            self.store.load()
        with self.assertRaises(ValueError):
            self.store.save(0, [PORT], {5: 512})
        self.assertEqual(self.path.read_text(), '{broken')

    def test_invalid_saved_values_are_rejected(self):
        self.path.parent.mkdir()
        for value in (1024, -1025, True, '512'):
            with self.subTest(value=value):
                self.path.write_text(json.dumps({'version': 1, 'last_gpu': 0,
                    'gpus': {'0': {'0:5:DisplayPort': value}}}))
                with self.assertRaises(ValueError):
                    self.store.load()

    def test_absolute_xdg_config_home_is_used(self):
        with patch.dict(os.environ, {'XDG_CONFIG_HOME': self.temp.name}):
            self.assertEqual(config_path(), Path(self.temp.name) / 'nvibrant/settings.json')
        with patch.dict(os.environ, {'XDG_CONFIG_HOME': 'relative/path'}):
            self.assertEqual(config_path(), Path.home() / '.config/nvibrant/settings.json')


if __name__ == '__main__':
    unittest.main()
