import os
import subprocess
import unittest
from unittest.mock import patch
from nvibrant.backend import Backend


class BackendTests(unittest.TestCase):
    def test_subprocess_is_bounded_and_uses_vibrance(self):
        backend = Backend(gpu=2)
        result = subprocess.CompletedProcess([], 0, 'Success', '')
        with patch('pathlib.Path.is_file', return_value=True), \
             patch.dict(os.environ, {'ATTRIBUTE': 'dithering'}), \
             patch('subprocess.run', return_value=result) as run:
            backend.run(512)
            self.assertEqual(run.call_args.kwargs['timeout'], 15)
            self.assertEqual(run.call_args.kwargs['env']['ATTRIBUTE'], 'vibrance')
            self.assertEqual(run.call_args.kwargs['env']['NVIDIA_GPU'], '2')
            self.assertEqual(run.call_args.args[0][-1], '512')

    def test_failed_driver_call_is_reported(self):
        backend = Backend()
        result = subprocess.CompletedProcess([], 1, 'AllocDevice ioctl failed', '')
        with patch('pathlib.Path.is_file', return_value=True), \
             patch('subprocess.run', return_value=result):
            with self.assertRaisesRegex(RuntimeError, 'AllocDevice'):
                backend.run('--list-json')

    def test_discovery_is_read_only(self):
        backend = Backend()
        with patch.object(backend, 'run', return_value='{"ports":[]}') as run:
            self.assertEqual(backend.discover(), [])
            run.assert_called_once_with('--list-json')

    def test_sparse_outputs_preserve_argument_positions(self):
        backend = Backend()
        with patch.object(backend, 'run', return_value='Success') as run:
            backend.apply([{'index': 0}, {'index': 3}], {3: 512})
            run.assert_called_once_with(0, 0, 0, 512)

    def test_each_connected_output_keeps_its_value(self):
        backend = Backend()
        with patch.object(backend, 'run', return_value='Success') as run:
            backend.apply([{'index': 0}, {'index': 2}], {0: -1024, 2: 1023})
            run.assert_called_once_with(-1024, 0, 1023)

    def test_no_success_is_an_error(self):
        backend = Backend()
        with patch.object(backend, 'run', return_value='None'):
            with self.assertRaises(RuntimeError):
                backend.apply([{'index': 0}], {0: 512})

    def test_invalid_value_never_reaches_driver(self):
        backend = Backend()
        with patch.object(backend, 'run') as run:
            with self.assertRaises(ValueError):
                backend.apply([{'index': 0}], {0: 1024})
            run.assert_not_called()

    def test_demo_never_invokes_hardware(self):
        backend = Backend(demo=True)
        with patch.object(backend, 'run') as run:
            ports = backend.discover()
            backend.apply(ports, {0: 512})
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
