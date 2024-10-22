use std::any::{Any, TypeId};

use crate::prelude::*;

/// 寄存器地址接口
pub trait IAddress {
    /// 获取完整的寄存器地址
    fn get_address_name(&self) -> &str;
    /// 获取寄存器地址头部
    fn get_address_header(&self) -> &str;
    /// 获取寄存器地址尾部
    fn get_address(&self) -> u32;
    /// 获取数据类型
    fn get_data_type(&self) -> &DataType;

    // 其他方法的定义,用于实现 downcast_ref
    fn as_any(&self) -> &dyn Any;
    fn type_id(&self) -> TypeId {
        self.as_any().type_id()
    }
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.type_id() == TypeId::of::<T>() {
            unsafe { Some(&*(self.as_any() as *const dyn Any as *const T)) }
        } else {
            None
        }
    }
}
