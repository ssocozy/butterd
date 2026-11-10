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
def platform_security_patch(bulletin, month):
    # Read only the patch-level cell of the matching Android bulletin row.
    # Publication dates and navigation links must never become patch metadata.
    for row in re.findall(r'<tr\b[^>]*>.*?</tr>', bulletin, re.S | re.I):
        if not re.search(r'/bulletin/\d{4}/' + re.escape(month) + r'-01(?:[".?/#])', row):
            continue
        cells = re.findall(r'<td\b[^>]*>(.*?)</td>', row, re.S | re.I)
        if not cells:
            continue
        text = html.unescape(re.sub(r'<[^>]*>', ' ', cells[-1])).strip()
        if re.fullmatch(r'(?:' + re.escape(month) + r'-(?:01|05)\s*)+', text):
            return max(re.findall(re.escape(month) + r'-(?:01|05)', text))
    return None
def verified_build(builds, device, bulletin, platform_bulletin=""):
    candidates = [build for build in builds if build.get('previewMetadata', {}).get('canary')
                  and build.get('product') == device + '_beta' and build.get('buildId', '').isdigit()]
    for build in sorted(candidates, key=lambda build: int(build['buildId']), reverse=True):
        period = re.fullmatch(r'canary-(\d{4})(\d{2})', build['previewMetadata'].get('id', ''))
        release = build.get('releaseCandidateName', '')
        if not period or not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', release):
            continue
        month = period[1] + '-' + period[2]
        patch = security_patch(bulletin, month) or platform_security_patch(platform_bulletin, month)
        if patch:
            return build, patch
    raise ValueError('No Canary build with a verified security patch')
def run():
    page = html.unescape(get('https://flash.android.com'))
    key = re.search(r'AIza[0-9A-Za-z_-]{30,}', page).group()
    try: bulletin = get('https://source.android.com/docs/security/bulletin/pixel')
    except Exception: bulletin = ''
    try: platform_bulletin = get('https://source.android.com/docs/security/bulletin/asb-overview')
    except Exception: platform_bulletin = ''
    count = 0
    for device, model in MODELS.items():
        try:
            builds = json.loads(get('https://content-flashstation-pa.googleapis.com/v1/builds?product='+device+'_beta&key='+key))['flashstationBuild']
            # Canary metadata can appear before its month's security bulletin.
            # Select the newest verifiable build instead of inventing its patch.
            build, patch = verified_build(builds, device, bulletin, platform_bulletin)
            release = build['releaseCandidateName']; incremental = build['buildId']
            content = f'FINGERPRINT=google/{device}_beta/{device}:CANARY/{release}/{incremental}:user/release-keys\nMANUFACTURER=Google\nMODEL={model}\n'
            content += 'SECURITY_PATCH='+patch+'\n'
            content += 'DEVICE_INITIAL_SDK_INT='+str(INITIAL_API[device])+'\n'
            content += 'spoofBuild=true\nspoofVendingBuild=true\nspoofProps=true\n'
            ROOT.mkdir(parents=True, exist_ok=True)
            (ROOT/(device+'.prop')).write_text(content)
            print('Updated',model);count += 1
        except Exception as failure: print('Retaining bundled profile for',model,':',type(failure).__name__)
    if not count: raise RuntimeError('No fresh profiles obtained; retaining bundled metadata')
if __name__ == '__main__':run()
