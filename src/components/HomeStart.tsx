import {ArrowRight,FileText,GitBranch,FolderOpen} from 'lucide-react';
export default function HomeStart({navigate}:{navigate:(page:string)=>void}){
 return <section className="home-start" aria-labelledby="home-start-title"><div><h2 id="home-start-title">继续工作</h2><p>打开项目、查看开发进展，或把本周成果整理成周报。</p></div><div className="home-shortcuts"><button onClick={()=>navigate('all')}><FolderOpen size={18}/><span>浏览全部项目</span><ArrowRight size={14}/></button><button onClick={()=>navigate('development')}><GitBranch size={18}/><span>查看开发状态</span><ArrowRight size={14}/></button><button onClick={()=>navigate('weekly-report')}><FileText size={18}/><span>开始整理周报</span><ArrowRight size={14}/></button></div></section>;
}
