#!/usr/bin/env python3
"""Read-only evaluation of pHash OR ORB+RANSAC on labeled pair CSVs."""
import argparse
from pathlib import Path
import pandas as pd

KEY=['first','second','same_family']

def load(phash, orb):
    p=pd.read_csv(phash); o=pd.read_csv(orb)
    for name, df, cols in [('pHash',p,KEY+['predicted']),('ORB',o,KEY+['predicted','ransac_inliers'])]:
        missing=set(cols)-set(df.columns)
        if missing: raise ValueError(f'{name}: missing columns {sorted(missing)}')
        if df.duplicated(KEY).any(): raise ValueError(f'{name}: duplicate pair keys')
    d=p.merge(o,on=KEY,how='outer',validate='one_to_one',indicator=True,suffixes=('_p','_o'))
    if not (d['_merge']=='both').all(): raise ValueError('pHash/ORB pair sets do not match')
    if not d.same_family.isin([0,1]).all() or not d.predicted_p.isin([0,1]).all() or not d.predicted_o.isin([0,1]).all(): raise ValueError('invalid boolean columns')
    return d

def metrics(d,t):
    y=d.same_family.eq(1)
    p=d.predicted_p.eq(1)
    o=d.predicted_o.eq(1)
    selected=p | (o & d.ransac_inliers.ge(t))
    tp=int((selected & y).sum()); fp=int((selected & ~y).sum()); fn=int((~selected & y).sum()); tn=int((~selected & ~y).sum())
    return dict(threshold=t,TP=tp,FP=fp,FN=fn,TN=tn,precision=round(tp/(tp+fp),6) if tp+fp else None,recall=round(tp/(tp+fn),6) if tp+fn else None)

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument('--phash-v1',type=Path,required=True);ap.add_argument('--orb-v1',type=Path,required=True)
    ap.add_argument('--phash-v2',type=Path,required=True);ap.add_argument('--orb-v2',type=Path,required=True)
    ap.add_argument('--output',type=Path,default=Path('threshold_comparison.csv'))
    a=ap.parse_args(); datasets=[load(a.phash_v1,a.orb_v1),load(a.phash_v2,a.orb_v2)]
    thresholds=[0,10,12,15,18,20,21,22,23,24,25,26,30,35,40,50,75,100]
    records=[]
    for t in thresholds:
        for name,d in zip(['v1','v2'],datasets): records.append(dict(dataset=name,**metrics(d,t)))
        records.append(dict(dataset='combined',**metrics(pd.concat(datasets,ignore_index=True),t)))
    pd.DataFrame(records).to_csv(a.output,index=False)
    print(pd.DataFrame(records).query("dataset == 'combined'").to_string(index=False))
    print(f'Wrote {a.output}')
if __name__=='__main__': main()
