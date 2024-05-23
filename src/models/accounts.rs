use gtk::{gio, glib, prelude::*, subclass::prelude::*};

use super::account::Account;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    pub struct AccountsModel(pub RefCell<Vec<Account>>);

    #[glib::object_subclass]
    impl ObjectSubclass for AccountsModel {
        const NAME: &'static str = "AccountsModel";
        type Type = super::AccountsModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for AccountsModel {}
    impl ListModelImpl for AccountsModel {
        fn item_type(&self) -> glib::Type {
            Account::static_type()
        }
        fn n_items(&self) -> u32 {
            self.0.borrow().len() as u32
        }
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.0
                .borrow()
                .get(position as usize)
                .map(|o| o.clone().upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct AccountsModel(ObjectSubclass<imp::AccountsModel>)
        @implements gio::ListModel;
}

impl AccountsModel {
    pub fn append(&self, account: &Account) {
        let pos = {
            let mut data = self.imp().0.borrow_mut();
            data.push(account.clone());
            (data.len() - 1) as u32
        };
        self.items_changed(pos, 0, 1);
    }

    pub fn splice(&self, accounts: &[Account]) {
        let len = accounts.len();
        let pos = {
            let mut data = self.imp().0.borrow_mut();
            let pos = data.len();
            data.extend_from_slice(accounts);
            pos as u32
        };
        self.items_changed(pos, 0, len as u32);
    }

    pub fn remove(&self, pos: u32) {
        self.imp().0.borrow_mut().remove(pos as usize);
        self.items_changed(pos, 1, 0);
    }

    pub fn find_by_id(&self, id: u32) -> Option<Account> {
        for pos in 0..self.n_items() {
            let account = self.item(pos).and_downcast::<Account>().unwrap();
            if account.id() == id {
                return Some(account);
            }
        }
        None
    }

    pub fn find_position_by_id(&self, id: u32) -> Option<u32> {
        for pos in 0..self.n_items() {
            let obj = self.item(pos)?;
            let account = obj.downcast_ref::<Account>().unwrap();
            if account.id() == id {
                return Some(pos);
            }
        }
        None
    }
}

impl Default for AccountsModel {
    fn default() -> Self {
        glib::Object::new()
    }
}
