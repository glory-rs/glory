"""Summarize unfiltered portfolio-v1 samples; keep per-round medians."""
import argparse, collections, json, pathlib, statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('evidence', type=pathlib.Path)
parser.add_argument('--out', required=True, type=pathlib.Path)
args = parser.parse_args()
evidence = json.loads(args.evidence.read_text(encoding='utf-8'))
groups = collections.defaultdict(list)
rounds = {}
for run in evidence['rounds']:
    for row in run['results']:
        groups[(run['n'], run['variant'], row['op'])].extend(row['samples'])
        rounds[(run['round'],run['n'],run['variant'],row['op'])] = row['samples']
def describe(samples):
    result = {'samples':len(samples)}
    for field in ['ScriptDuration','wall_ms','TaskDuration','LayoutDuration','RecalcStyleDuration']:
        values = sorted(s[field] for s in samples)
        def quantile(p):
            index=(len(values)-1)*p;low=int(index);high=min(low+1,len(values)-1)
            return values[low]+(values[high]-values[low])*(index-low)
        result[field]={'median':statistics.median(values),'p10':quantile(.1),'p90':quantile(.9)}
    return result
summary={'contract':evidence['contract'],'unfiltered':True,'cells':[
    {'n':n,'variant':variant,'op':op,**describe(samples),'rounds':[
        {'round':r,**describe(rounds[(r,n,variant,op)])} for r in sorted(set(run['round'] for run in evidence['rounds']))
    ]} for (n,variant,op),samples in groups.items()]}
args.out.write_text(json.dumps(summary,indent=2),encoding='utf-8')
for n in sorted(set(k[0] for k in groups)):
    print('ROWS',n,'Script medians ms: Glory / Dioxus root / Dioxus row')
    for op in [r['op'] for r in evidence['rounds'][0]['results']]:
        print(op,' / '.join(f"{statistics.median(x['ScriptDuration'] for x in groups[(n,v,op)]):.3f}" for v in ['glory','dioxus-root','dioxus-row']))
print('Total samples',sum(len(v) for v in groups.values()))
