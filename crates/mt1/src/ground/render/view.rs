use super::*;
impl Paint<'_> {
    fn cell(&mut self,addr:Far,row:i32,col:i32,mut sy:i32,mut sx:i32,mut h:i32,mut w:i32,oy:i32,ox:i32) {
        let (f,tile,f2)=(self.g.mem.read(addr,1) as u8,self.g.mem.read(addr.add(1),1) as i32,self.g.mem.read(addr.add(2),1) as u8);
        let (mut y,mut x)=(row*16+oy+54,(col*2+ox)*8);
        let (sheet,index)=if f&1!=0 {let b=self.g.mem.u8(0x6FB3);("X.DAT",if b.eq_ignore_ascii_case(&b'I') {3} else {self.g.mem.u8(0x6FB4).wrapping_sub(b'1') as usize})} else {("OBJ.DAT",4)};
        self.picture(sheet,index,y,x,tile/20*16+sy,tile%20*16+sx*8,h,w*8,None);
        if f2&128!=0 {self.picture("OBJ.DAT",3,y,x,sy,sx*8,h,w*8,Some(1))}
        if f2&64!=0 {self.cpic(1,y,x,sy,sx*8,h,w*8)}
        let mut walker=f>>1;
        if walker!=0 && sy==0 && sx==0 {
            let mut seen=[false;128];
            while walker!=0 && walker<128 && !seen[walker as usize] {
                seen[walker as usize]=true;let p=self.g.mem.pointer(0x6B6E).add(walker as i32*9);
                self.ly("GROUND",self.g.mem.read(p.add(3),1) as usize,y,x);walker=self.g.mem.read(p.add(2),1) as u8;
            }
        }
        let i=f2&63;if i==0 {return}let obj=0x6422+(i as u16-1)*46;let slot=self.g.mem.u8(obj+8) as i32;
        if self.g.mem.u16(obj+32)&1!=0 {if h!=16 && row!=0 {return}if sy!=0 {sy=0;h=16} else {if row==6 {h=8}y+=8}}
        if self.g.mem.u16(obj+34)&1!=0 {if w!=2 && col!=0 {return}if sx!=0 {sx=0;w=2} else {if col==11 {w=1}x+=8}}
        let pose=self.g.mem.i8(obj+40) as i32;sx=(sx+(self.g.mem.u8(obj+36)&7) as i32*2+(pose+1)*16)*8;
        self.overlays.push([slot,y,x,sy,sx,h,w*8]);
        let p=self.g.mem.pointer(0x6B57);
        if p.seg==Mem::DS && p.off>=0x6B9B && p.off<=0xFFF1 {
            for (i,v) in [sy,sx/8,h,w,slot,y,x/8].into_iter().enumerate() {self.g.mem.write(p.add(i as i32*2),v as u32,2)}
            self.g.mem.set_pointer(0x6B57,p.add(14));
        }
    }
    fn map(&mut self) {
        if matches!(self.g.mem.u8(0xE8),1|2) && self.g.mem.party_u16(0xF82)==0 {self.g.mem.set_pointer(0x6B57,Mem::ds(0x6B9B));self.fill(54,0,112,192,0);return}
        let (r0,c0,stride,base)=(self.g.mem.i16(0x6415) as i32,self.g.mem.i16(0x6266) as i32,self.g.mem.map_stride() as i32,self.g.mem.pointer(0xBC));
        if self.g.mem.turning()!=0 && self.g.mem.u16(0x503E)!=0 {
            self.load_overlays();
            let (r,c)=(self.g.mem.x() as i32,self.g.mem.y() as i32);
            self.cell(base.add(((r>>1)*stride+(c>>1))*3),(r-r0)>>1,(c-c0)>>1,0,0,16,2,if r0&1!=0 {8} else {0},c0&1);return
        }
        if (r0,c0)==(self.g.mem.i16(0x6B73) as i32,self.g.mem.i16(0x6413) as i32) {
            copy(self.g,0x7158,0,0,112,24,0x503A,54,0);self.load_overlays();return
        }
        self.g.mem.set_pointer(0x6B57,Mem::ds(0x6B9B));
        for r in 0..7+(r0&1) {
            let sy=if r==0 && r0&1!=0 {8} else {0};let h=if r0&1!=0 && (r==0 || r==7) {8} else {16};
            let (rr,oy)=if r0&1!=0 && r!=0 {(r-1,8)} else {(r,0)};
            for c in 0..12+(c0&1) {
                let sx=i32::from(c==0 && c0&1!=0);let w=if c0&1!=0 && (c==0 || c==12) {1} else {2};
                let (cc,ox)=if c0&1!=0 && c!=0 {(c-1,1)} else {(c,0)};
                self.cell(base.add((((r0>>1)+r)*stride+(c0>>1)+c)*3),rr,cc,sy,sx,h,w,oy,ox);
            }
        }
        if (r0,c0)!=(self.g.mem.i16(0x6B73) as i32,self.g.mem.i16(0x6413) as i32) {
            copy(self.g,0x503A,54,0,112,24,0x7158,0,0);self.g.mem.set_i16(0x6B73,r0 as i16);self.g.mem.set_i16(0x6413,c0 as i16);
        }
    }
    fn load_overlays(&mut self) {
        let end=self.g.mem.u16(0x6B57);
        for a in (0x6B9Bu16..end.min(0xFFF2)).step_by(14) {if a as u32+14>end as u32 {break}let v:Vec<i32>=(0..7).map(|i|self.g.mem.i16(a+i*2) as i32).collect();self.overlays.push([v[4],v[5],v[6]*8,v[0],v[1]*8,v[2],v[3]*8]);}
    }
    fn walkers(&mut self) {
        let vehicle=self.g.mem.combat()!=0;let leader=self.g.mem.leader();
        if vehicle {
            let w=Mem::walker(leader);let (row,col,dir)=(self.g.mem.x(),self.g.mem.y(),self.g.mem.heading());
            if self.g.mem.individual()==0 {if self.g.mem.orders()==0 {self.g.mem.write(w.add(8),row as u32,2);self.g.mem.write(w.add(10),col as u32,2);self.g.mem.write(w.add(1),dir as u32,1)} else {self.g.mem.write(w.add(16),row as u32,2);self.g.mem.write(w.add(18),col as u32,2)}}
            else {let target=self.g.mem.read(w.add(5),1) as u8 as i8 as i16;let (r,c)=if target!=0 {let n=Mem::npc(target);(self.g.mem.read(n.add(32),2) as u16&!1,self.g.mem.read(n.add(34),2) as u16&!1)} else {(row as u16,col as u16)};self.g.mem.write(w.add(12),r as u32,2);self.g.mem.write(w.add(14),c as u32,2);}
        }
        let mut last=(self.g.mem.i16(0x285) as i32,self.g.mem.i16(0x287) as i32);let mut animation=self.g.mem.phase() as i32;
        for member in 0..if vehicle {5} else {1} {
            let w=0x5C54+member*86;let mut phase=animation;
            let (mut row,mut col,mut direction)=(self.g.mem.x() as i32,self.g.mem.y() as i32,self.g.mem.heading() as i32);
            let mut pic=usize::from(self.g.mem.encounter_mode()!=0);
            if vehicle {
                if self.g.mem.u8(w)==0 {continue}pic=2;
                if member as i16!=leader || self.g.mem.individual()!=0 {
                    row=self.g.mem.i16(w+8) as i32;col=self.g.mem.i16(w+10) as i32;direction=self.g.mem.u8(w+1) as i32;
                    if member as i16==leader && self.g.mem.orders()==0 && self.g.mem.individual()==0 {row=self.g.mem.x() as i32;col=self.g.mem.y() as i32;direction=self.g.mem.heading() as i32;}
                }
                if member as i16!=leader {phase=self.g.mem.i16(w+20) as i32}
            }
            let mut sx=direction*16;
            let moving=(!vehicle || member as i16==leader) && last!=(row,col) && !(vehicle && self.g.mem.npc_mode()==0 && self.g.mem.individual()!=0);
            if phase!=0 {sx+=(if self.g.mem.encounter_mode()!=0 {animation-1} else {phase})*128;if moving {animation^=3;self.g.mem.set_phase(animation as i16)}}
            self.g.mem.set_i16(0x285,row as i16);self.g.mem.set_i16(0x287,col as i16);self.g.mem.set_u16(0xE3,moving as u16);
            last=(row,col);let y=(row-self.g.mem.i16(0x6415) as i32)*8+54;let x=(col-self.g.mem.i16(0x6266) as i32)*8;
            self.cpic(pic,y,x,0,sx,16,16);if vehicle {self.pen.colour=member as u8+1;self.ly("GROUND",49,y+6,x+5)}
        }
        if vehicle && self.g.mem.npc_mode()==0 && self.g.mem.individual()==1 {
            let w=Mem::walker(leader);let target=self.g.mem.read(w.add(5),1) as u16;
            let (row,col)=if target==0 {(self.g.mem.x(),self.g.mem.y())} else {(self.g.mem.i16(0x63F4+target*46+32)&!1,self.g.mem.i16(0x63F4+target*46+34)&!1)};
            let y=(row as i32-self.g.mem.i16(0x6415) as i32)*8+54;let x=(col as i32-self.g.mem.i16(0x6266) as i32)*8;
            self.rect(y+1,x+1,y+15,x+15,0);self.rect(y,x,y+14,x+14,leader.wrapping_add(1) as u8);
        }
    }
}
pub fn redraw(g:&mut Ground) {
    g.observe_boot("redraw",&[]);g.enter("redraw_view");
    let now=g.now();let page=g.mem.u16(0x52F4)&1;
    let mut p=Paint::new(g,Raster::Clip);p.page=(page^1) as usize;p.map();
    for [slot,y,x,sy,sx,h,w] in std::mem::take(&mut p.overlays) {p.cpic(slot as usize,y,x,sy,sx,h,w)}
    p.g.enter("draw_walkers");p.walkers();p.finish();p.g.frame("draw_walkers");let n=p.g.mem.countdown();
    if p.g.mem.u8(0xE5+page)!=n && p.g.mem.combat()==0 {
        if n==99 {p.side_panel(false)}
        if n!=100 {p.ly("LED",n as usize/10,68,200);p.ly("LED",n as usize%10,68,226)} else {p.ly("LED",12,68,200);p.side_panel(false)}
    }
    p.finish();
    if g.mem.combat()==0 {g.mem.set_u8(0xE5+page,n)}
    if g.mem.combat()!=0 && g.mem.npc_mode()==0 && g.mem.individual()!=0 && now>g.mem.target_deadline() {g.mem.set_individual(g.mem.individual()^3);g.mem.set_target_deadline(now.wrapping_add(9));}
    flip(g);frame(g,"redraw_view");
}
