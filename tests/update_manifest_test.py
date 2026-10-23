#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('update_manifest', Path(__file__).resolve().parents[1] / 'tools/update_manifest.py')
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


class UpdateManifestTest(unittest.TestCase):
    def test_update_targets_exact_production_zip_and_matching_notes(self):
        result = updater.manifest({'version': '0.4.26', 'versionCode': '36'}, 'ssocozy/butterd')
        self.assertEqual(result['versionCode'], 36)
        base = 'https://github.com/ssocozy/butterd/releases/download/v0.4.26/'
        self.assertEqual(result['zipUrl'], base + 'butterd-0.4.26.zip')
        self.assertEqual(result['changelog'], base + 'changelog.txt')
        self.assertNotIn('sources', result['zipUrl'])

    def test_reject_invalid_version_and_repository(self):
        for version, code, repo in [('dev', '36', 'ssocozy/butterd'), ('0.4.26', '0', 'ssocozy/butterd'), ('0.4.26', '36', 'ssocozy/butterd?token=secret')]:
            with self.assertRaises(ValueError):
                updater.manifest({'version': version, 'versionCode': code}, repo)


if __name__ == '__main__':
    unittest.main()
