#!/usr/bin/env python3
"""Fetch metadata only from Google's Flash Tool. No images or executable code."""
import html
import json
import re
import urllib.request
from pathlib import Path
MODELS = {'bluejay':'Pixel 6a','panther':'Pixel 7','cheetah':'Pixel 7 Pro','lynx':'Pixel 7a','shiba':'Pixel 8','husky':'Pixel 8 Pro','akita':'Pixel 8a','tokay':'Pixel 9','caiman':'Pixel 9 Pro','komodo':'Pixel 9 Pro XL','tegu':'Pixel 9a'}
ROOT = Path(__file__).resolve().parents[1] / 'module/compatibility/profiles'
def get(url):
    request = urllib.request.Request(url, headers={'Referer':'https://flash.android.com'})
    with urllib.request.urlopen(request, timeout=12) as response:
        body = response.read(4 * 1024 * 1024 + 1)
    if len(body) > 4 * 1024 * 1024: raise ValueError('Metadata exceeds limit')
    return body.decode()
def run():
    page = html.unescape(get('https://flash.android.com'))
    key = re.search(r'AIza[0-9A-Za-z_-]{30,}', page).group()
    try: bulletin = get('https://source.android.com/docs/security/bulletin/pixel')
    except Exception: bulletin = ''
    count = 0
    for device, model in MODELS.items():
        try:
            builds = json.loads(get('https://content-flashstation-pa.googleapis.com/v1/builds?product='+device+'_beta&key='+key))['flashstationBuild']
            candidates = [b for b in builds if b.get('previewMetadata',{}).get('canary') and b.get('product') == device+'_beta' and b.get('buildId','').isdigit()]
            build = max(candidates, key=lambda b:int(b['buildId']))
            release = build['releaseCandidateName']; incremental = build['buildId']
            if not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', release): raise ValueError('Invalid release identifier')
            content = f'FINGERPRINT=google/{device}_beta/{device}:CANARY/{release}/{incremental}:user/release-keys\nMANUFACTURER=Google\nMODEL={model}\n'
            period = re.fullmatch(r'canary-(\d{4})(\d{2})',build['previewMetadata'].get('id',''))
            if period:
                month = period[1]+'-'+period[2]
                match = re.search(r'<td[^>]*>\s*'+month+r'\s*</td>\s*<td[^>]*>\s*('+month+r'-\d{2})\s*</td>',bulletin)
                if match:content += 'SECURITY_PATCH='+match[1]+'\n'
            content += 'spoofBuild=true\nspoofVendingBuild=true\nspoofProps=true\n'
            ROOT.mkdir(parents=True, exist_ok=True)
            (ROOT/(device+'.prop')).write_text(content)
            print('Updated',model);count += 1
        except Exception as failure: print('Retaining bundled profile for',model,':',type(failure).__name__)
    if not count: raise RuntimeError('No fresh profiles obtained; retaining bundled metadata')
if __name__ == '__main__':run()
