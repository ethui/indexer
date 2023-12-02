use std::str::FromStr;

use bigdecimal::BigDecimal;
use diesel::sql_types::Numeric;
use diesel::{
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    pg::{Pg, PgValue},
    serialize::{self, Output, ToSql},
    sql_types::Bytea,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, AsExpression, FromSqlRow, Clone)]
#[diesel(sql_type=Bytea)]
pub struct Address(pub alloy_primitives::Address);

#[derive(Debug, Deserialize, Serialize, AsExpression, FromSqlRow)]
#[diesel(sql_type=Numeric)]
pub struct U256(pub alloy_primitives::U256);

#[derive(Debug, Deserialize, Serialize, AsExpression, FromSqlRow)]
#[diesel(sql_type=Bytea)]
pub struct B256(pub alloy_primitives::B256);

impl From<alloy_primitives::Address> for Address {
    fn from(value: alloy_primitives::Address) -> Self {
        Self(value)
    }
}

impl From<alloy_primitives::U256> for U256 {
    fn from(value: alloy_primitives::U256) -> Self {
        Self(value)
    }
}

impl From<alloy_primitives::B256> for B256 {
    fn from(value: alloy_primitives::B256) -> Self {
        Self(value)
    }
}

impl ToSql<Bytea, Pg> for Address {
    fn to_sql(&self, out: &mut Output<'_, '_, Pg>) -> serialize::Result {
        <Vec<u8> as ToSql<Bytea, Pg>>::to_sql(&self.0.to_vec(), &mut out.reborrow())
    }
}

impl FromSql<Bytea, Pg> for Address {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        <Vec<u8> as FromSql<Bytea, Pg>>::from_sql(bytes)
            .map(|b| Address(alloy_primitives::Address::from_slice(&b)))
    }
}

impl ToSql<Numeric, Pg> for U256 {
    fn to_sql(&self, out: &mut Output<'_, '_, Pg>) -> serialize::Result {
        let decimal = BigDecimal::from_str(&self.0.to_string())?;
        <BigDecimal as ToSql<Numeric, Pg>>::to_sql(&decimal, &mut out.reborrow())
    }
}

impl FromSql<Numeric, Pg> for U256 {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bigdecimal = <BigDecimal as FromSql<Numeric, Pg>>::from_sql(bytes)?;

        Ok(Self(alloy_primitives::U256::from_str(
            &bigdecimal.to_string(),
        )?))
    }
}

impl ToSql<Bytea, Pg> for B256 {
    fn to_sql(&self, out: &mut Output<'_, '_, Pg>) -> serialize::Result {
        <Vec<u8> as ToSql<Bytea, Pg>>::to_sql(&self.0.to_vec(), &mut out.reborrow())
    }
}

impl FromSql<Bytea, Pg> for B256 {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        <Vec<u8> as FromSql<Bytea, Pg>>::from_sql(bytes)
            .map(|b| B256(alloy_primitives::B256::from_slice(&b)))
    }
}
