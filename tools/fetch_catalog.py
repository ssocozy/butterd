#!/usr/bin/env python3
"""Fetch metadata only from Google's Flash Tool. No images or executable code."""
import html
import json
import re
import urllib.request
from pathlib import Path
MODELS = {'bluejay':'Pixel 6a','panther':'Pixel 7','cheetah':'Pixel 7 Pro','lynx':'Pixel 7a','shiba':'Pixel 8','husky':'Pixel 8 Pro','akita':'Pixel 8a','tokay':'Pixel 9','caiman':'Pixel 9 Pro','komodo':'Pixel 9 Pro XL','tegu':'Pixel 9a'}
INITIAL_API = dict(zip(MODELS, (32,33,33,33,34,34,34,34,34,34,35)))
ROOT = Path(__file__).resolve().parents[1] / 'module/compatibility/profiles'
def get(url):
    request = urllib.request.Request(url, headers={'Referer':'https://flash.android.com'})
    with urllib.request.urlopen(request, timeout=12) as response:
        body = response.read(4 * 1024 * 1024 + 1)
    if len(body) > 4 * 1024 * 1024: raise ValueError('Metadata exceeds limit')
    return body.decode()
def security_patch(bulletin, month):
    # The month is in the bulletin link, not an adjacent plain-text table cell.
    for row in re.findall(r'<tr\b[^>]*>.*?</tr>', bulletin, re.S | re.I):
        if not re.search(r'/pixel/\d{4}/'+re.escape(month)+r'-\d{2}(?:[".?/#])', row):
            continue
        cells = re.findall(r'<td\b[^>]*>(.*?)</td>', row, re.S | re.I)
        if not cells: continue
        value = html.unescape(re.sub(r'<[^>]*>', '', cells[-1])).strip()
        if re.fullmatch(re.escape(month)+r'-(?:0[1-9]|[12][0-9]|3[01])', value): return value
    return None
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
            if not period: raise ValueError('Unknown Canary month')
            if period:
                month = period[1]+'-'+period[2]
                patch = security_patch(bulletin, month)
                if not patch: raise ValueError('No verified security patch for Canary month')
                content += 'SECURITY_PATCH='+patch+'\n'
            content += 'DEVICE_INITIAL_SDK_INT='+str(INITIAL_API[device])+'\n'
            content += 'spoofBuild=true\nspoofVendingBuild=true\nspoofProps=true\n'
            ROOT.mkdir(parents=True, exist_ok=True)
            (ROOT/(device+'.prop')).write_text(content)
            print('Updated',model);count += 1
        except Exception as failure: print('Retaining bundled profile for',model,':',type(failure).__name__)
    if not count: raise RuntimeError('No fresh profiles obtained; retaining bundled metadata')
if __name__ == '__main__':run()
