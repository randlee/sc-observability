"""Portable regression checks for Windows controller input/failure handling."""
import base64
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _windows_identity import powershell, Identity


class ControllerTests(unittest.TestCase):
    def test_multiline_script_is_one_encoded_command_without_pwsh_module_paths(self):
        script="Add-Type -TypeDefinition 'line one\nline two'"
        result=subprocess.CompletedProcess([],0,'result\n','')
        with patch.dict(os.environ,{'PSModulePath':'foreign-pwsh-modules'}):
            with patch('subprocess.run',return_value=result) as run:
                self.assertEqual(powershell(script),'result')
        args,kwargs=run.call_args
        self.assertIn('-EncodedCommand',args[0])
        self.assertEqual(base64.b64decode(args[0][-1]).decode('utf-16le'),
                         "$ErrorActionPreference='Stop';\n"+script)
        self.assertNotIn('psmodulepath',{key.casefold() for key in kwargs['env']})
        self.assertEqual(kwargs['timeout'],60)

    def test_account_creation_error_does_not_echo_credentials(self):
        result=subprocess.CompletedProcess([],1,'','temporary-secret')
        with patch('subprocess.run',return_value=result):
            with self.assertRaises(RuntimeError) as failure:
                powershell('temporary-secret',sensitive=True)
        self.assertNotIn('temporary-secret',str(failure.exception))

    def test_timeout_does_not_echo_encoded_credentials(self):
        error=subprocess.TimeoutExpired(['powershell','encoded-secret'],60)
        with patch('subprocess.run',side_effect=error):
            with self.assertRaisesRegex(RuntimeError,'exceeded 60 seconds') as failure:
                powershell('temporary-secret',sensitive=True)
        self.assertNotIn('secret',str(failure.exception))
        self.assertTrue(failure.exception.__suppress_context__)

    def test_unprotected_proof_launch_fails_before_process_creation(self):
        identity=Identity.__new__(Identity)
        identity.active=False
        with self.assertRaisesRegex(RuntimeError,'before network policy'):
            identity.run(['unavailable-proof-program'])


if __name__=='__main__': unittest.main()
