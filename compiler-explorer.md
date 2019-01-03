```
#[repr(C)]
pub struct OtherThing {
    a: u32,
}

#[repr(C)]
pub struct Thing {
    one: u32,
    other: Box<OtherThing>,
}

const other_offset: usize = ::std::mem::size_of::<Thing>();

fn serialize(v: &mut [u8]) {
    unsafe {
        let t: &Thing = ::std::mem::transmute::<_, &Thing>(v.get_unchecked_mut(0));
        let t_ptr = (t as *const Thing);
        // let mut ot_ptr = ::std::mem::transmute::<_, *const OtherThing>(&t.other);
        
        let mut ot_ptr = ::std::mem::transmute::<_, *mut usize>(&t.other);
        *ot_ptr = other_offset;
    }
}

#[inline(always)]
pub fn get_other_thing(v: &mut [u8] /* Thing, followed by all the reachable pointees */) -> &OtherThing {
    unsafe {
        let t: &Thing = ::std::mem::transmute::<_, &Thing>(v.get_unchecked_mut(0));
        let t_ptr = (t as *const Thing);
        let mut ot_ptr = ::std::mem::transmute::<_, *mut usize>(&t.other);
        ((t_ptr as *const u8).add(*ot_ptr) as *const OtherThing).as_ref().unwrap()
    }
}

pub fn do_thing(v: &mut [u8]) {
    let other_thing_a = get_other_thing(v).a;
    println!("{}", other_thing_a);
}

pub fn do_thing_right(t: Thing) {
    let other_thing_a = t.other.a;
    println!("{}", other_thing_a);
}

fn main() {
    let mut v = Vec::with_capacity(1024);
    let t = Thing {
        one: 12,
        other: Box::new(OtherThing {
            a: 33,
        }),
    };
    let tslice = unsafe {
        ::std::slice::from_raw_parts(::std::mem::transmute(&t), ::std::mem::size_of::<Thing>())
    };
    use std::io::Write;
    v.write(tslice).unwrap();
    let totherslice = unsafe {
        ::std::slice::from_raw_parts(::std::mem::transmute::<&OtherThing, _>(&t.other), ::std::mem::size_of::<OtherThing>())
    };
    v.write(totherslice).unwrap();
    serialize(&mut v);
    do_thing(&mut v);
}
```
